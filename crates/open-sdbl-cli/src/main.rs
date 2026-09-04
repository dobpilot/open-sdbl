use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::future::Future;
use std::io::{self, BufWriter, IsTerminal, Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{Stream, StreamExt};
use open_sdbl::metadata::{
    LiveColumn, LiveIndex, LiveTable, MetadataError, MetadataSnapshot, MsSqlMetadataQueries,
    PostgresMetadataQueries, parse_config_descriptors, parse_config_predefined_values,
    parse_db_names, parse_schema_storage, resolve_metadata_with_predefined_values,
};
use open_sdbl::query::MsSqlBackend;
use open_sdbl::{Diagnostic, tokenize};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{WebPkiServerVerifier, verify_server_cert_signed_by_trust_anchor};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{ClientConfig as RustlsClientConfig, DigitallySignedStruct, RootCertStore};
use tiberius::{
    AuthMethod, Client as MsSqlClient, ColumnType as MsSqlColumnType, Config as MsSqlConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_postgres::config::SslMode;
use tokio_postgres::tls::MakeTlsConnect;
use tokio_postgres::types::ToSql;
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};
use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zeroize::Zeroizing;

mod repl;

#[cfg(test)]
#[path = "../../../tests/support/hex.rs"]
mod hex_test_support;

const HELP: &str = "open-sdbl — tooling for the 1C query language\n\n\
Usage:\n  open-sdbl lex [FILE|-]\n  open-sdbl metadata postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl metadata mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl --help\n\n\
Commands:\n  lex       Print lexical tokens; reads standard input when FILE is '-' or omitted\n  metadata  Read and resolve 1C information-base metadata\n  console   Run 1C queries and inspect resolved metadata interactively\n\n\
PostgreSQL options:\n  --port PORT                 PostgreSQL port (default: 5432)\n  --sslmode MODE              disable, require, verify-ca, or verify-full (default)\n  --insecure-plaintext        Required explicit opt-in for --sslmode disable\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy\n  --socks5-user USER          Authenticate to SOCKS5 using SOCKS5_PASSWORD\n\n\
MSSQL options:\n  --port PORT                 SQL Server port (default: 1433)\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy\n  --socks5-user USER          Authenticate to SOCKS5 using SOCKS5_PASSWORD\n  --trust-server-certificate  Accept any TLS certificate (unsafe; development only)\n  --trust-ca-file PATH        Trust a specific PEM, CRT, or DER certificate\n\n\
Authentication:\n  PostgreSQL: PGPASSWORD, PGPASSFILE, or $HOME/.pgpass\n  MSSQL: MSSQL_PASSWORD\n  SOCKS5: SOCKS5_PASSWORD (when --socks5-user is present)\n\n\
Read-only requirements:\n  MSSQL login must belong to db_datareader, but not db_datawriter, db_owner, or sysadmin\n";

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(120);
const POSTGRES_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const PROGRESS_REDRAW_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_BAR_WIDTH: usize = 24;
const CONFIG_DECODE_BATCH_SIZE: usize = 256;
const MAX_PRINTED_ROWS: usize = 1_000;
const MAX_CELL_WIDTH: usize = 256;
const INSECURE_MSSQL_CERTIFICATE_WARNING: &str = "warning: --trust-server-certificate disables MSSQL certificate and hostname verification; prefer --trust-ca-file";
const MSSQL_VERIFY_READONLY: &str = "SELECT CONVERT(int, @@TRANCOUNT), CONVERT(int, CASE WHEN ISNULL(IS_MEMBER(N'db_datareader'), 0) = 1 AND ISNULL(IS_MEMBER(N'db_datawriter'), 0) = 0 AND ISNULL(IS_MEMBER(N'db_owner'), 0) = 0 AND ISNULL(IS_SRVROLEMEMBER(N'sysadmin'), 0) = 0 THEN 1 ELSE 0 END), CONVERT(int, transaction_isolation_level) FROM sys.dm_exec_sessions WHERE session_id = @@SPID";
const MSSQL_TRANSACTION_COUNT: &str = "SELECT CONVERT(int, @@TRANCOUNT)";

struct MetadataProgress {
    enabled: bool,
    active: bool,
    phase: &'static str,
    completed_resources: u64,
    total_resources: u64,
    completed_bytes: u64,
    total_bytes: u64,
    started: Instant,
    last_draw: Option<Instant>,
}

impl MetadataProgress {
    fn new() -> Self {
        Self {
            enabled: io::stderr().is_terminal(),
            active: false,
            phase: "starting",
            completed_resources: 0,
            total_resources: 0,
            completed_bytes: 0,
            total_bytes: 0,
            started: Instant::now(),
            last_draw: None,
        }
    }

    #[cfg(test)]
    fn disabled() -> Self {
        let mut progress = Self::new();
        progress.enabled = false;
        progress
    }

    fn phase(&mut self, phase: &'static str) {
        self.phase = phase;
        self.draw(true);
    }

    fn config_totals(&mut self, resources: u64, bytes: u64) {
        self.total_resources = resources;
        self.total_bytes = bytes;
        self.phase("Config");
    }

    fn advance_config(&mut self, resources: usize, bytes: usize) {
        self.completed_resources = self.completed_resources.saturating_add(resources as u64);
        self.completed_bytes = self.completed_bytes.saturating_add(bytes as u64);
        self.draw(false);
    }

    fn finish(mut self) {
        if !self.enabled {
            return;
        }
        self.phase = "complete";
        self.completed_resources = self.total_resources;
        self.completed_bytes = self.total_bytes;
        let line = render_metadata_progress(
            self.phase,
            self.completed_resources,
            self.total_resources,
            self.completed_bytes,
            self.total_bytes,
            PROGRESS_BAR_WIDTH,
        );
        let mut stderr = io::stderr().lock();
        let _ = writeln!(
            stderr,
            "\r\x1b[2K{line} in {}",
            format_elapsed(self.started.elapsed())
        );
        let _ = stderr.flush();
        self.active = false;
    }

    fn draw(&mut self, force: bool) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if !force
            && self
                .last_draw
                .is_some_and(|last| now.duration_since(last) < PROGRESS_REDRAW_INTERVAL)
        {
            return;
        }
        self.last_draw = Some(now);
        self.active = true;
        let line = render_metadata_progress(
            self.phase,
            self.completed_resources,
            self.total_resources,
            self.completed_bytes,
            self.total_bytes,
            PROGRESS_BAR_WIDTH,
        );
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "\r\x1b[2K{line}");
        let _ = stderr.flush();
    }
}

impl Drop for MetadataProgress {
    fn drop(&mut self) {
        if self.enabled && self.active {
            let mut stderr = io::stderr().lock();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        }
    }
}

fn render_metadata_progress(
    phase: &str,
    completed_resources: u64,
    total_resources: u64,
    completed_bytes: u64,
    total_bytes: u64,
    width: usize,
) -> String {
    let ratio = if total_bytes != 0 {
        completed_bytes as f64 / total_bytes as f64
    } else if total_resources != 0 {
        completed_resources as f64 / total_resources as f64
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    let filled = ((ratio * width as f64).floor() as usize).min(width);
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(width - filled));
    let resources = if total_resources == 0 {
        format!("{completed_resources}/?")
    } else {
        format!("{completed_resources}/{total_resources}")
    };
    let bytes = if total_bytes == 0 {
        format!("{}/?", format_bytes(completed_bytes))
    } else {
        format!(
            "{}/{}",
            format_bytes(completed_bytes),
            format_bytes(total_bytes)
        )
    };
    format!(
        "metadata [{bar}] {:>5.1}% {phase} {resources} {bytes}",
        ratio * 100.0
    )
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn format_elapsed(elapsed: Duration) -> String {
    if elapsed.as_secs() != 0 {
        format!("{:.1} s", elapsed.as_secs_f64())
    } else {
        format!("{} ms", elapsed.as_millis())
    }
}

#[derive(Clone, Debug)]
enum EnvironmentSecret {
    Missing,
    InvalidUnicode,
    Present(Zeroizing<String>),
}

impl EnvironmentSecret {
    fn take(name: &'static str) -> Self {
        let value = env::var_os(name);
        // SAFETY: this is called before the Tokio runtime and any application
        // worker threads are created. No other thread can concurrently access
        // the process environment at this point in this binary.
        unsafe { env::remove_var(name) };
        match value {
            None => Self::Missing,
            Some(value) => value
                .into_string()
                .map(Zeroizing::new)
                .map_or(Self::InvalidUnicode, Self::Present),
        }
    }

    fn optional(&self, name: &str) -> Result<Option<Zeroizing<String>>, CliError> {
        match self {
            Self::Missing => Ok(None),
            Self::InvalidUnicode => Err(CliError::Data(format!("{name} is not valid UTF-8"))),
            Self::Present(value) => Ok(Some(value.clone())),
        }
    }

    fn required(&self, name: &str) -> Result<Zeroizing<String>, CliError> {
        self.optional(name)?.ok_or_else(|| {
            CliError::Data(format!(
                "{name} is required for the selected authentication method"
            ))
        })
    }
}

struct Credentials {
    postgres: EnvironmentSecret,
    mssql: EnvironmentSecret,
    socks5: EnvironmentSecret,
}

impl Credentials {
    fn take_from_environment() -> Self {
        Self {
            postgres: EnvironmentSecret::take("PGPASSWORD"),
            mssql: EnvironmentSecret::take("MSSQL_PASSWORD"),
            socks5: EnvironmentSecret::take("SOCKS5_PASSWORD"),
        }
    }
}

fn main() -> ExitCode {
    let credentials = Credentials::take_from_environment();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("cannot start Tokio runtime: {error}");
            return ExitCode::from(2);
        }
    };
    runtime.block_on(async_main(credentials))
}

async fn async_main(credentials: Credentials) -> ExitCode {
    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let result = run(&mut output, &credentials).await.and_then(|()| {
        output
            .flush()
            .map_err(|error| CliError::Io("cannot flush standard output".to_owned(), error))
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.is_broken_pipe() => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", escape_field(&error.to_string()));
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run(output: &mut impl Write, credentials: &Credentials) -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" => {
            output
                .write_all(HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            Ok(())
        }
        "lex" => {
            let path = arguments.next().unwrap_or_else(|| "-".to_owned());
            if let Some(unexpected) = arguments.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument {unexpected:?}\n\n{HELP}"
                )));
            }
            let source = read_lex_source(&path)?;
            let tokens = tokenize(&source).map_err(CliError::Lexical)?;
            lex(output, &tokens).map_err(CliError::standard_output)
        }
        "metadata" => metadata(arguments, output, credentials).await,
        "console" | "repl" => console(arguments, output, credentials).await,
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

async fn metadata(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "metadata", output)? else {
        return Ok(());
    };

    let mut session = DatabaseSession::connect(&connection, credentials).await?;
    let result = session.metadata().await;
    let close_result = session.close().await;
    let snapshot = result?;
    print_snapshot(output, &snapshot).map_err(CliError::standard_output)?;
    if let Err(error) = close_result {
        eprintln!(
            "warning: metadata was loaded, but the database session did not close cleanly: {}",
            escape_field(&error.to_string())
        );
    }
    Ok(())
}

async fn console(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "console", output)? else {
        return Ok(());
    };
    let mut session = DatabaseSession::connect(&connection, credentials).await?;
    let result = async {
        let snapshot = session.metadata().await?;
        repl::run(&mut session, snapshot, output).await
    }
    .await;
    let close_result = session.close().await;
    result?;
    close_result
}

fn parse_connection(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
    output: &mut impl Write,
) -> Result<Option<DatabaseConnection>, CliError> {
    let Some(provider) = arguments.next() else {
        return Err(CliError::Usage(format!(
            "missing {command} provider\n\n{HELP}"
        )));
    };
    if matches!(provider.as_str(), "-h" | "--help") {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(None);
    }
    if !matches!(provider.as_str(), "postgres" | "mssql") {
        return Err(CliError::Usage(format!(
            "unsupported {command} provider {provider:?}\n\n{HELP}"
        )));
    }

    let mut options = ConnectionOptions {
        host: String::new(),
        port: if provider == "postgres" { 5432 } else { 1433 },
        database: String::new(),
        user: String::new(),
        socks5_proxy: None,
    };
    let mut trust_server_certificate = false;
    let mut trust_ca_file = None;
    let mut postgres_sslmode = None;
    let mut insecure_plaintext = false;
    let mut socks5_user = None;
    while let Some(option) = arguments.next() {
        if matches!(option.as_str(), "-h" | "--help") {
            output
                .write_all(HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            return Ok(None);
        }
        if option == "--trust-server-certificate" {
            if provider != "mssql" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
            eprintln!("{INSECURE_MSSQL_CERTIFICATE_WARNING}");
            trust_server_certificate = true;
            continue;
        }
        if option == "--insecure-plaintext" {
            if provider != "postgres" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
            insecure_plaintext = true;
            continue;
        }
        let value = arguments
            .next()
            .ok_or_else(|| CliError::Usage(format!("missing value for {option:?}\n\n{HELP}")))?;
        match option.as_str() {
            "--host" => options.host = value,
            "--database" => options.database = value,
            "--user" => options.user = value,
            "--port" => {
                options.port = value.parse().map_err(|_| {
                    CliError::Usage(format!("invalid {provider} port {value:?}\n\n{HELP}"))
                })?;
            }
            "--socks5-proxy" => {
                options.socks5_proxy = Some(parse_socks5_proxy(&value).map_err(|reason| {
                    CliError::Usage(format!(
                        "invalid SOCKS5 proxy {value:?}: {reason}\n\n{HELP}"
                    ))
                })?);
            }
            "--socks5-user" => socks5_user = Some(value),
            "--sslmode" if provider == "postgres" => {
                postgres_sslmode = Some(parse_postgres_sslmode(&value).map_err(|reason| {
                    CliError::Usage(format!(
                        "invalid PostgreSQL sslmode {value:?}: {reason}\n\n{HELP}"
                    ))
                })?);
            }
            "--trust-ca-file" if provider == "mssql" => trust_ca_file = Some(value),
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
        }
    }
    if options.host.is_empty() || options.database.is_empty() || options.user.is_empty() {
        return Err(CliError::Usage(format!(
            "--host, --database, and --user are required\n\n{HELP}"
        )));
    }
    if socks5_user.is_some() && options.socks5_proxy.is_none() {
        return Err(CliError::Usage(format!(
            "--socks5-user requires --socks5-proxy\n\n{HELP}"
        )));
    }
    if let Some(proxy) = options.socks5_proxy.as_mut() {
        proxy.username = socks5_user;
    }

    Ok(Some(if provider == "postgres" {
        let sslmode = resolve_postgres_sslmode(postgres_sslmode)?;
        if sslmode == PostgresSslMode::Disable && !insecure_plaintext {
            return Err(CliError::Usage(format!(
                "--sslmode disable requires --insecure-plaintext\n\n{HELP}"
            )));
        }
        if sslmode != PostgresSslMode::Disable && insecure_plaintext {
            return Err(CliError::Usage(format!(
                "--insecure-plaintext is valid only with --sslmode disable\n\n{HELP}"
            )));
        }
        DatabaseConnection::Postgres(PostgresConnection { options, sslmode })
    } else {
        if trust_server_certificate && trust_ca_file.is_some() {
            return Err(CliError::Usage(format!(
                "--trust-server-certificate and --trust-ca-file are mutually exclusive\n\n{HELP}"
            )));
        }
        DatabaseConnection::MsSql(MsSqlConnection {
            options,
            trust_server_certificate,
            trust_ca_file,
        })
    }))
}

#[derive(Debug)]
enum DatabaseConnection {
    Postgres(PostgresConnection),
    MsSql(MsSqlConnection),
}

#[derive(Clone, Debug)]
struct ConnectionOptions {
    host: String,
    port: u16,
    database: String,
    user: String,
    socks5_proxy: Option<Socks5Proxy>,
}

#[derive(Clone, Debug)]
struct PostgresConnection {
    options: ConnectionOptions,
    sslmode: PostgresSslMode,
}

impl std::ops::Deref for PostgresConnection {
    type Target = ConnectionOptions;

    fn deref(&self) -> &Self::Target {
        &self.options
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostgresSslMode {
    Disable,
    Require,
    VerifyCa,
    VerifyFull,
}

fn parse_postgres_sslmode(value: &str) -> Result<PostgresSslMode, &'static str> {
    match value {
        "disable" => Ok(PostgresSslMode::Disable),
        "require" => Ok(PostgresSslMode::Require),
        "verify-ca" => Ok(PostgresSslMode::VerifyCa),
        "verify-full" => Ok(PostgresSslMode::VerifyFull),
        _ => Err("expected disable, require, verify-ca, or verify-full"),
    }
}

fn resolve_postgres_sslmode(
    explicit: Option<PostgresSslMode>,
) -> Result<PostgresSslMode, CliError> {
    let environment = if explicit.is_none() {
        match env::var("PGSSLMODE") {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(CliError::Usage(format!(
                    "PGSSLMODE is not valid UTF-8\n\n{HELP}"
                )));
            }
        }
    } else {
        None
    };
    select_postgres_sslmode(explicit, environment.as_deref())
        .map_err(|reason| CliError::Usage(format!("invalid PGSSLMODE: {reason}\n\n{HELP}")))
}

fn select_postgres_sslmode(
    explicit: Option<PostgresSslMode>,
    environment: Option<&str>,
) -> Result<PostgresSslMode, String> {
    if let Some(mode) = explicit {
        return Ok(mode);
    }
    environment.map_or(Ok(PostgresSslMode::VerifyFull), |value| {
        parse_postgres_sslmode(value).map_err(|reason| format!("{value:?}: {reason}"))
    })
}

#[derive(Debug)]
struct PostgresServerCertVerifier {
    certificate_roots: Option<Arc<RootCertStore>>,
    signature_verifier: Arc<WebPkiServerVerifier>,
}

impl ServerCertVerifier for PostgresServerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if let Some(roots) = &self.certificate_roots {
            let certificate = ParsedCertificate::try_from(end_entity)?;
            let provider = rustls::crypto::ring::default_provider();
            verify_server_cert_signed_by_trust_anchor(
                &certificate,
                roots,
                intermediates,
                now,
                provider.signature_verification_algorithms.all,
            )?;
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.signature_verifier
            .verify_tls12_signature(message, certificate, signature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.signature_verifier
            .verify_tls13_signature(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.signature_verifier.supported_verify_schemes()
    }
}

fn postgres_tls_connector(mode: PostgresSslMode) -> Result<MakeRustlsConnect, CliError> {
    let signature_roots = Arc::new(RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    });
    let signature_verifier = WebPkiServerVerifier::builder(Arc::clone(&signature_roots))
        .build()
        .map_err(|error| CliError::Data(format!("cannot configure PostgreSQL TLS: {error}")))?;

    let certificate_roots = match mode {
        PostgresSslMode::Require => None,
        PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull => {
            let native_certificates = rustls_native_certs::load_native_certs();
            let mut roots = RootCertStore::empty();
            let (accepted, _) = roots.add_parsable_certificates(native_certificates.certs);
            if accepted == 0 {
                let details = native_certificates
                    .errors
                    .first()
                    .map_or_else(String::new, |error| format!(": {error}"));
                return Err(CliError::Data(format!(
                    "cannot load any native CA certificates for PostgreSQL TLS{details}"
                )));
            }
            Some(Arc::new(roots))
        }
        PostgresSslMode::Disable => {
            return Err(CliError::Data(
                "internal error: TLS connector requested for plaintext PostgreSQL".to_owned(),
            ));
        }
    };
    let roots = certificate_roots
        .as_deref()
        .cloned()
        .unwrap_or_else(|| (*signature_roots).clone());
    let mut configuration = RustlsClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    match mode {
        PostgresSslMode::Require => configuration.dangerous().set_certificate_verifier(Arc::new(
            PostgresServerCertVerifier {
                certificate_roots: None,
                signature_verifier,
            },
        )),
        PostgresSslMode::VerifyCa => configuration.dangerous().set_certificate_verifier(Arc::new(
            PostgresServerCertVerifier {
                certificate_roots,
                signature_verifier,
            },
        )),
        PostgresSslMode::VerifyFull => {}
        PostgresSslMode::Disable => unreachable!("handled before TLS configuration"),
    }
    Ok(MakeRustlsConnect::new(configuration))
}

#[derive(Clone, Debug)]
struct MsSqlConnection {
    options: ConnectionOptions,
    trust_server_certificate: bool,
    trust_ca_file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Socks5Proxy {
    host: String,
    port: u16,
    username: Option<String>,
}

fn parse_socks5_proxy(value: &str) -> Result<Socks5Proxy, &'static str> {
    let (host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let (host, port) = bracketed.split_once("]:").ok_or("expected [IPv6]:PORT")?;
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => (host, port),
            _ => return Err("brackets are only valid around an IPv6 address"),
        }
    } else {
        let (host, port) = value.rsplit_once(':').ok_or("expected HOST:PORT")?;
        if host.contains(':') {
            return Err("IPv6 addresses must be enclosed in brackets");
        }
        (host, port)
    };

    if host.is_empty() || host.trim() != host {
        return Err("host must not be empty or contain surrounding whitespace");
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| "port must be an integer from 1 to 65535")?;
    if port == 0 {
        return Err("port must be an integer from 1 to 65535");
    }
    Ok(Socks5Proxy {
        host: host.to_owned(),
        port,
        username: None,
    })
}

async fn bounded_database_call<T>(
    label: &str,
    duration: Duration,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    timeout(duration, future).await.map_err(|_| {
        CliError::Database(format!(
            "{label} timed out after {:.3} seconds",
            duration.as_secs_f64()
        ))
    })?
}

async fn query_timeout<T>(
    label: &str,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    bounded_database_call(label, QUERY_TIMEOUT, future).await
}

struct PostgresSession {
    client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    connection: PostgresConnection,
    socks5_password: Option<Zeroizing<String>>,
}

impl PostgresSession {
    async fn connect(
        connection: &PostgresConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        let mut configuration = tokio_postgres::Config::new();
        configuration
            .host(&connection.host)
            .port(connection.port)
            .dbname(&connection.database)
            .user(&connection.user)
            .connect_timeout(CONNECTION_TIMEOUT)
            .options(format!(
                "-c statement_timeout={}",
                QUERY_TIMEOUT.as_millis()
            ))
            .ssl_mode(match connection.sslmode {
                PostgresSslMode::Disable => SslMode::Disable,
                PostgresSslMode::Require
                | PostgresSslMode::VerifyCa
                | PostgresSslMode::VerifyFull => SslMode::Require,
            });
        if let Some(password) = postgres_password(connection, &credentials.postgres)? {
            configuration.password(password.as_str());
        }
        let socks5_password =
            socks5_password(connection.socks5_proxy.as_ref(), &credentials.socks5)?;

        let (client, driver) = match (connection.sslmode, &connection.socks5_proxy) {
            (PostgresSslMode::Disable, Some(proxy)) => {
                let stream = connect_socks5(
                    proxy,
                    socks5_password.as_deref().map(String::as_str),
                    &connection.host,
                    connection.port,
                )
                .await?;
                connect_postgres_raw(&configuration, stream, CONNECTION_TIMEOUT).await?
            }
            (PostgresSslMode::Disable, None) => {
                let (client, connection_driver) = configuration
                    .connect(NoTls)
                    .await
                    .map_err(CliError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
            (mode, Some(proxy)) => {
                let stream = connect_socks5(
                    proxy,
                    socks5_password.as_deref().map(String::as_str),
                    &connection.host,
                    connection.port,
                )
                .await?;
                connect_postgres_raw_tls(
                    &configuration,
                    stream,
                    postgres_tls_connector(mode)?,
                    &connection.host,
                    CONNECTION_TIMEOUT,
                )
                .await?
            }
            (mode, None) => {
                let (client, connection_driver) = configuration
                    .connect(postgres_tls_connector(mode)?)
                    .await
                    .map_err(CliError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
        };
        Ok(Self {
            client,
            driver,
            connection: connection.clone(),
            socks5_password,
        })
    }

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        let transaction = query_timeout("PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(CliError::from)
        })
        .await?;
        let snapshot = acquire_metadata(&transaction).await;
        match snapshot {
            Ok(snapshot) => {
                query_timeout("PostgreSQL transaction commit", async {
                    transaction.commit().await.map_err(CliError::from)
                })
                .await?;
                Ok(snapshot)
            }
            Err(error) => {
                let _ = query_timeout("PostgreSQL transaction rollback", async {
                    transaction.rollback().await.map_err(CliError::from)
                })
                .await;
                Err(error)
            }
        }
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, CliError> {
        let transaction = query_timeout("PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(CliError::from)
        })
        .await?;
        if let Err(error) = verify_transaction(&transaction).await {
            let _ = query_timeout("PostgreSQL transaction rollback", async {
                transaction.rollback().await.map_err(CliError::from)
            })
            .await;
            return Err(error);
        }
        let query = query_timeout("PostgreSQL query", async {
            transaction.query(sql, &[]).await.map_err(CliError::from)
        })
        .await;
        match query {
            Ok(rows) => {
                let rows = rows
                    .iter()
                    .map(|row| {
                        (0..column_count)
                            .map(|index| row.try_get(index).map_err(CliError::from))
                            .collect()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                query_timeout("PostgreSQL transaction commit", async {
                    transaction.commit().await.map_err(CliError::from)
                })
                .await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = query_timeout("PostgreSQL transaction rollback", async {
                    transaction.rollback().await.map_err(CliError::from)
                })
                .await;
                Err(error)
            }
        }
    }

    async fn cancel_query(&self, token: tokio_postgres::CancelToken) -> Result<(), CliError> {
        let connection = &self.connection;
        query_timeout("PostgreSQL query cancellation", async {
            match (connection.sslmode, &connection.socks5_proxy) {
                (PostgresSslMode::Disable, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                    )
                    .await?;
                    token
                        .cancel_query_raw(stream, NoTls)
                        .await
                        .map_err(CliError::database_connection)
                }
                (PostgresSslMode::Disable, None) => token
                    .cancel_query(NoTls)
                    .await
                    .map_err(CliError::database_connection),
                (mode, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                    )
                    .await?;
                    let mut tls = postgres_tls_connector(mode)?;
                    let tls = <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(
                        &mut tls,
                        &connection.host,
                    )
                    .map_err(|error| {
                        CliError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
                    })?;
                    token
                        .cancel_query_raw(stream, tls)
                        .await
                        .map_err(CliError::database_connection)
                }
                (mode, None) => token
                    .cancel_query(postgres_tls_connector(mode)?)
                    .await
                    .map_err(CliError::database_connection),
            }
        })
        .await
    }

    async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        await_postgres_driver(self.driver, POSTGRES_CLOSE_TIMEOUT).await
    }
}

async fn await_postgres_driver(
    mut driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    close_timeout: Duration,
) -> Result<(), CliError> {
    match timeout(close_timeout, &mut driver).await {
        Ok(result) => result
            .map_err(|error| {
                CliError::Database(format!("PostgreSQL connection task failed: {error}"))
            })?
            .map_err(CliError::database_connection),
        Err(_) => {
            driver.abort();
            let _ = driver.await;
            Err(CliError::Database(format!(
                "PostgreSQL connection close timed out after {:.3} seconds; driver aborted",
                close_timeout.as_secs_f64()
            )))
        }
    }
}

pub(crate) type QueryRows = Vec<Vec<Option<String>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatabaseDialect {
    Postgres,
    MsSql { backend: MsSqlBackend },
}

enum DatabaseSession {
    Postgres(Box<PostgresSession>),
    MsSql(Box<MsSqlSession>),
}

pub(crate) enum QueryCancellation {
    Postgres(tokio_postgres::CancelToken),
    MsSql,
}

impl DatabaseSession {
    async fn connect(
        connection: &DatabaseConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        match connection {
            DatabaseConnection::Postgres(connection) => {
                PostgresSession::connect(connection, credentials)
                    .await
                    .map(Box::new)
                    .map(Self::Postgres)
            }
            DatabaseConnection::MsSql(connection) => MsSqlSession::connect(connection, credentials)
                .await
                .map(Box::new)
                .map(Self::MsSql),
        }
    }

    pub(crate) const fn dialect(&self) -> DatabaseDialect {
        match self {
            Self::Postgres(_) => DatabaseDialect::Postgres,
            Self::MsSql(session) => DatabaseDialect::MsSql {
                backend: session.backend,
            },
        }
    }

    pub(crate) const fn execution_label(&self) -> &'static str {
        match self {
            Self::Postgres(_) => "PostgreSQL execution",
            Self::MsSql(_) => "MSSQL execution",
        }
    }

    pub(crate) fn is_dead(&self) -> bool {
        match self {
            Self::Postgres(session) => session.client.is_closed(),
            Self::MsSql(session) => session.poisoned,
        }
    }

    pub(crate) fn cancellation(&self) -> QueryCancellation {
        match self {
            Self::Postgres(session) => QueryCancellation::Postgres(session.client.cancel_token()),
            Self::MsSql(_) => QueryCancellation::MsSql,
        }
    }

    pub(crate) async fn cancel_query(
        &mut self,
        cancellation: QueryCancellation,
    ) -> Result<(), CliError> {
        match (self, cancellation) {
            (Self::Postgres(session), QueryCancellation::Postgres(token)) => {
                session.cancel_query(token).await
            }
            (Self::MsSql(session), QueryCancellation::MsSql) => {
                session.cancel_and_reconnect().await
            }
            _ => Err(CliError::Database(
                "database session changed while cancelling a query".to_owned(),
            )),
        }
    }

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        match self {
            Self::Postgres(session) => session.metadata().await,
            Self::MsSql(session) => session.metadata().await,
        }
    }

    pub(crate) async fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> Result<QueryRows, CliError> {
        match self {
            Self::Postgres(session) => session.query(sql, column_count).await,
            Self::MsSql(session) => session.query(sql, column_count).await,
        }
    }

    async fn close(self) -> Result<(), CliError> {
        match self {
            Self::Postgres(session) => session.close().await,
            Self::MsSql(session) => session.close().await,
        }
    }
}

type MsSqlTransport = Compat<TcpStream>;

#[derive(Clone)]
struct MsSqlSecrets {
    password: Zeroizing<String>,
    socks5_password: Option<Zeroizing<String>>,
}

struct MsSqlSession {
    client: Option<MsSqlClient<MsSqlTransport>>,
    connection: MsSqlConnection,
    database: String,
    backend: MsSqlBackend,
    poisoned: bool,
    secrets: MsSqlSecrets,
}

impl MsSqlSession {
    async fn connect(
        connection: &MsSqlConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        let secrets = MsSqlSecrets {
            password: credentials.mssql.required("MSSQL_PASSWORD")?,
            socks5_password: socks5_password(
                connection.options.socks5_proxy.as_ref(),
                &credentials.socks5,
            )?,
        };
        Self::connect_with_secrets(connection, secrets).await
    }

    async fn connect_with_secrets(
        connection: &MsSqlConnection,
        secrets: MsSqlSecrets,
    ) -> Result<Self, CliError> {
        let options = &connection.options;
        let mut configuration = MsSqlConfig::new();
        configuration.host(&options.host);
        configuration.port(options.port);
        configuration.database(&options.database);
        configuration.authentication(AuthMethod::sql_server(
            &options.user,
            secrets.password.as_str(),
        ));
        configuration.application_name("open-sdbl");
        configuration.readonly(true);
        if connection.trust_server_certificate {
            configuration.trust_cert();
        } else if let Some(path) = &connection.trust_ca_file {
            configuration.trust_cert_ca(path);
        }

        let stream = if let Some(proxy) = &options.socks5_proxy {
            connect_socks5(
                proxy,
                secrets.socks5_password.as_deref().map(String::as_str),
                &options.host,
                options.port,
            )
            .await?
        } else {
            timeout(
                CONNECTION_TIMEOUT,
                TcpStream::connect((options.host.as_str(), options.port)),
            )
            .await
            .map_err(|_| {
                CliError::Database(format!(
                    "MSSQL TCP connection timed out after {} seconds",
                    CONNECTION_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|error| {
                CliError::Io("cannot connect to MSSQL TCP endpoint".to_owned(), error)
            })?
        };
        stream.set_nodelay(true).map_err(|error| {
            CliError::Io("cannot configure MSSQL TCP connection".to_owned(), error)
        })?;
        let client = timeout(
            CONNECTION_TIMEOUT,
            MsSqlClient::connect(configuration, stream.compat_write()),
        )
        .await
        .map_err(|_| {
            CliError::Database(format!(
                "MSSQL startup timed out after {} seconds",
                CONNECTION_TIMEOUT.as_secs()
            ))
        })?
        .map_err(CliError::mssql_connection)?;
        let mut session = Self {
            client: Some(client),
            connection: connection.clone(),
            database: options.database.clone(),
            backend: MsSqlBackend::default(),
            poisoned: false,
            secrets,
        };
        session
            .execute_batch(
                &format!(
                    "SET QUOTED_IDENTIFIER ON; SET TRANSACTION ISOLATION LEVEL READ COMMITTED; SET LOCK_TIMEOUT {};",
                    QUERY_TIMEOUT.as_millis()
                ),
            )
            .await?;
        session.verify_database().await?;
        let year_offset = session.read_year_offset().await?;
        session.backend =
            MsSqlBackend::new(year_offset).map_err(|error| CliError::Data(error.to_string()))?;
        Ok(session)
    }

    fn client_mut(&mut self) -> Result<&mut MsSqlClient<MsSqlTransport>, CliError> {
        self.client.as_mut().ok_or_else(|| {
            CliError::Database("MSSQL connection is closed; reconnect required".to_owned())
        })
    }

    async fn execute_batch(&mut self, sql: &str) -> Result<(), CliError> {
        self.ensure_usable()?;
        let client = self.client_mut()?;
        let result = query_timeout("MSSQL batch", async {
            client
                .simple_query(sql)
                .await
                .map_err(CliError::mssql_query)?
                .into_results()
                .await
                .map_err(CliError::mssql_query)?;
            Ok(())
        })
        .await;
        if result
            .as_ref()
            .is_err_and(CliError::is_mssql_connection_failure)
        {
            self.poisoned = true;
        }
        result
    }

    async fn verify_database(&mut self) -> Result<(), CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL database verification",
            MsSqlMetadataQueries::VERIFY_DATABASE,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "database verification")?;
        let actual = required_mssql_string(row, 0, "database name")?;
        let status = required_mssql_string(row, 1, "database status")?;
        if !actual.eq_ignore_ascii_case(&self.database) || !status.eq_ignore_ascii_case("ONLINE") {
            return Err(CliError::Data(format!(
                "unexpected MSSQL database state: database={actual:?}, status={status:?}"
            )));
        }
        Ok(())
    }

    async fn read_year_offset(&mut self) -> Result<i32, CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL _YearOffset query",
            MsSqlMetadataQueries::YEAR_OFFSET,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "_YearOffset")?;
        let offset = row
            .try_get::<i32, _>(0)
            .map_err(CliError::mssql_query)?
            .ok_or_else(|| {
                CliError::Data("MSSQL returned NULL for _YearOffset.Offset".to_owned())
            })?;
        if !matches!(offset, 0 | 2000) {
            return Err(CliError::Data(format!(
                "unsupported MSSQL _YearOffset.Offset value {offset}; expected 0 or 2000"
            )));
        }
        Ok(offset)
    }

    fn ensure_usable(&self) -> Result<(), CliError> {
        if self.poisoned || self.client.is_none() {
            Err(CliError::Database(
                "MSSQL session is poisoned and cannot be reused; reconnect required".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    async fn verify_readonly(&mut self) -> Result<(), CliError> {
        self.ensure_usable()?;
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL read-only verification",
            MSSQL_VERIFY_READONLY,
        )
        .await;
        if rows
            .as_ref()
            .is_err_and(CliError::is_mssql_connection_failure)
        {
            self.poisoned = true;
        }
        let rows = rows?;
        let row = exactly_one_mssql_row(&rows, "read-only verification")?;
        let transaction_count = required_mssql_i32(row, 0, "@@TRANCOUNT")?;
        let read_only = required_mssql_i32(row, 1, "read-only role result")?;
        let isolation = required_mssql_i32(row, 2, "transaction isolation level")?;
        if transaction_count != 0 || read_only != 1 || isolation != 2 {
            return Err(CliError::Data(format!(
                "unsafe MSSQL session: transaction_count={transaction_count}, db_datareader_only={}, isolation_level={isolation}; use a login in db_datareader and not db_datawriter/db_owner/sysadmin",
                read_only == 1
            )));
        }
        Ok(())
    }

    async fn transaction_count(&mut self) -> Result<i32, CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL transaction-state verification",
            MSSQL_TRANSACTION_COUNT,
        )
        .await?;
        required_mssql_i32(
            exactly_one_mssql_row(&rows, "transaction-state verification")?,
            0,
            "@@TRANCOUNT",
        )
    }

    async fn rollback_after_error(&mut self, original: CliError) -> CliError {
        if original.is_mssql_connection_failure() {
            self.poisoned = true;
        }
        let rollback = self
            .execute_batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION")
            .await
            .map_err(|error| error.to_string());
        let transaction_count = if rollback.is_ok() {
            self.transaction_count()
                .await
                .map_err(|error| error.to_string())
        } else {
            Ok(0)
        };
        match apply_mssql_cleanup(&mut self.poisoned, rollback, transaction_count) {
            Ok(()) => original,
            Err(cleanup) => CliError::Database(format!(
                "{original}; MSSQL rollback cleanup failed and the session was poisoned: {cleanup}"
            )),
        }
    }

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        self.verify_readonly().await?;
        self.execute_batch("BEGIN TRANSACTION").await?;
        let result = acquire_mssql_metadata(self.client_mut()?).await;
        match result {
            Ok(snapshot) => match self.execute_batch("COMMIT TRANSACTION").await {
                Ok(()) => Ok(snapshot),
                Err(error) => Err(self.rollback_after_error(error).await),
            },
            Err(error) => Err(self.rollback_after_error(error).await),
        }
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, CliError> {
        self.verify_readonly().await?;
        self.execute_batch("BEGIN TRANSACTION").await?;
        let client = self.client_mut()?;
        let result = query_timeout("MSSQL user query", async {
            let rows = client
                .simple_query(sql)
                .await
                .map_err(CliError::mssql_query)?
                .into_first_result()
                .await
                .map_err(CliError::mssql_query)?;
            rows.iter()
                .map(|row| {
                    (0..column_count)
                        .map(|index| mssql_cell_text(row, index))
                        .collect()
                })
                .collect()
        })
        .await;
        match result {
            Ok(rows) => match self.execute_batch("COMMIT TRANSACTION").await {
                Ok(()) => Ok(rows),
                Err(error) => Err(self.rollback_after_error(error).await),
            },
            Err(error) => Err(self.rollback_after_error(error).await),
        }
    }

    async fn cancel_and_reconnect(&mut self) -> Result<(), CliError> {
        let rollback = if self.client.is_some() {
            self.execute_batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION")
                .await
                .err()
        } else {
            None
        };
        self.poisoned = true;
        drop(self.client.take());
        let connection = self.connection.clone();
        let replacement = Self::connect_with_secrets(&connection, self.secrets.clone()).await?;
        *self = replacement;
        if let Some(error) = rollback {
            eprintln!(
                "warning: MSSQL rollback after cancellation failed; the connection was dropped and replaced: {}",
                escape_field(&error.to_string())
            );
        }
        Ok(())
    }

    async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        Ok(())
    }
}

fn mssql_cell_text(row: &tiberius::Row, index: usize) -> Result<Option<String>, CliError> {
    let column = row.columns().get(index).ok_or_else(|| {
        CliError::Data(format!(
            "MSSQL returned {} columns, but column {index} was requested",
            row.columns().len()
        ))
    })?;
    match column.column_type() {
        MsSqlColumnType::BigVarBin | MsSqlColumnType::BigBinary | MsSqlColumnType::Image => row
            .try_get::<&[u8], _>(index)
            .map(|value| value.map(format_mssql_binary))
            .map_err(CliError::mssql_query),
        _ => row
            .try_get::<&str, _>(index)
            .map(|value| value.map(str::to_owned))
            .map_err(CliError::mssql_query),
    }
}

fn format_mssql_binary(value: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(2 + value.len() * 2);
    output.push_str("0x");
    for byte in value {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
}

async fn connect_postgres_raw(
    configuration: &tokio_postgres::Config,
    stream: TcpStream,
    connect_timeout: Duration,
) -> Result<
    (
        tokio_postgres::Client,
        tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    ),
    CliError,
> {
    match timeout(connect_timeout, configuration.connect_raw(stream, NoTls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(CliError::database_connection(error)),
        Err(_) => Err(CliError::Database(format!(
            "PostgreSQL startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}

async fn connect_postgres_raw_tls(
    configuration: &tokio_postgres::Config,
    stream: TcpStream,
    mut tls: MakeRustlsConnect,
    hostname: &str,
    connect_timeout: Duration,
) -> Result<
    (
        tokio_postgres::Client,
        tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    ),
    CliError,
> {
    let tls =
        <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(&mut tls, hostname)
            .map_err(|error| {
                CliError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
            })?;
    match timeout(connect_timeout, configuration.connect_raw(stream, tls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(CliError::database_connection(error)),
        Err(_) => Err(CliError::Database(format!(
            "PostgreSQL TLS startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}

fn socks5_password(
    proxy: Option<&Socks5Proxy>,
    environment: &EnvironmentSecret,
) -> Result<Option<Zeroizing<String>>, CliError> {
    let Some(username) = proxy.and_then(|proxy| proxy.username.as_ref()) else {
        return Ok(None);
    };
    if username.is_empty() || username.len() > usize::from(u8::MAX) {
        return Err(CliError::Data(
            "SOCKS5 username must contain from 1 to 255 bytes".to_owned(),
        ));
    }
    let password = environment.required("SOCKS5_PASSWORD")?;
    if password.is_empty() || password.len() > usize::from(u8::MAX) {
        return Err(CliError::Data(
            "SOCKS5_PASSWORD must contain from 1 to 255 bytes".to_owned(),
        ));
    }
    Ok(Some(password))
}

async fn connect_socks5(
    proxy: &Socks5Proxy,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, CliError> {
    let request =
        socks5_connect_request(target_host, target_port).map_err(CliError::socks5_connection)?;
    let negotiation = async {
        let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port)).await?;

        if proxy.username.is_some() {
            stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await?;
        } else {
            stream.write_all(&[0x05, 0x01, 0x00]).await?;
        }
        let mut method = [0_u8; 2];
        stream.read_exact(&mut method).await?;
        match method {
            [0x05, 0x00] => {}
            [0x05, 0x02] => {
                let username = proxy.username.as_deref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "proxy requires SOCKS5 username/password authentication",
                    )
                })?;
                let password = password.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "SOCKS5 password is unavailable",
                    )
                })?;
                let username_length = u8::try_from(username.len()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 username is too long")
                })?;
                let password_length = u8::try_from(password.len()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 password is too long")
                })?;
                let mut authentication =
                    Zeroizing::new(Vec::with_capacity(username.len() + password.len() + 3));
                authentication.extend_from_slice(&[0x01, username_length]);
                authentication.extend_from_slice(username.as_bytes());
                authentication.push(password_length);
                authentication.extend_from_slice(password.as_bytes());
                stream.write_all(authentication.as_slice()).await?;
                let mut response = [0_u8; 2];
                stream.read_exact(&mut response).await?;
                match response {
                    [0x01, 0x00] => {}
                    [0x01, _] => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "proxy rejected SOCKS5 username/password authentication",
                        ));
                    }
                    [version, _] => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "proxy returned unexpected SOCKS5 authentication version 0x{version:02x}"
                            ),
                        ));
                    }
                }
            }
            [0x05, 0xff] => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "proxy rejected unauthenticated SOCKS5 access",
                ));
            }
            [0x05, selected] => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("proxy selected unsupported authentication method 0x{selected:02x}"),
                ));
            }
            [version, _] => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("proxy returned unexpected SOCKS version 0x{version:02x}"),
                ));
            }
        }

        stream.write_all(&request).await?;
        let mut response = [0_u8; 4];
        stream.read_exact(&mut response).await?;
        if response[0] != 0x05 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "proxy returned unexpected SOCKS version 0x{:02x}",
                    response[0]
                ),
            ));
        }
        if response[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                socks5_reply_message(response[1]),
            ));
        }
        if response[2] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proxy returned a malformed SOCKS5 response",
            ));
        }

        let bound_address_len = match response[3] {
            0x01 => 4,
            0x03 => {
                let mut length = [0_u8; 1];
                stream.read_exact(&mut length).await?;
                usize::from(length[0])
            }
            0x04 => 16,
            address_type => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("proxy returned unknown address type 0x{address_type:02x}"),
                ));
            }
        };
        let mut bound_address_and_port = vec![0_u8; bound_address_len + 2];
        stream.read_exact(&mut bound_address_and_port).await?;
        Ok(stream)
    };

    match timeout(CONNECTION_TIMEOUT, negotiation).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => Err(CliError::socks5_connection(error)),
        Err(_) => Err(CliError::socks5_connection(format!(
            "timed out after {} seconds",
            CONNECTION_TIMEOUT.as_secs()
        ))),
    }
}

fn socks5_connect_request(target_host: &str, target_port: u16) -> io::Result<Vec<u8>> {
    let mut request = Vec::with_capacity(target_host.len() + 8);
    request.extend_from_slice(&[0x05, 0x01, 0x00]);
    match target_host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            request.push(0x01);
            request.extend_from_slice(&address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            request.push(0x04);
            request.extend_from_slice(&address.octets());
        }
        Err(_) => {
            let length = u8::try_from(target_host.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "PostgreSQL hostname is too long for SOCKS5",
                )
            })?;
            request.extend_from_slice(&[0x03, length]);
            request.extend_from_slice(target_host.as_bytes());
        }
    }
    request.extend_from_slice(&target_port.to_be_bytes());
    Ok(request)
}

fn socks5_reply_message(reply: u8) -> String {
    let reason = match reply {
        0x01 => "general proxy failure",
        0x02 => "connection not allowed by proxy rules",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown proxy error",
    };
    format!("proxy rejected CONNECT request: {reason} (0x{reply:02x})")
}

async fn acquire_metadata(transaction: &Transaction<'_>) -> Result<MetadataSnapshot, CliError> {
    let mut progress = MetadataProgress::new();
    progress.phase("transaction");
    verify_transaction(transaction).await?;

    progress.phase("DBNames");
    let db_names_rows = postgres_rows(
        transaction,
        "PostgreSQL DBNames query",
        PostgresMetadataQueries::DB_NAMES,
    )
    .await?;
    let db_names_data: Vec<u8> = exactly_one_row(&db_names_rows, "DBNames")?.try_get(0)?;
    let db_names = run_metadata_blocking("DBNames", move || {
        parse_db_names(&db_names_data).map_err(CliError::from)
    })
    .await?;

    let totals = query_timeout("PostgreSQL Config totals query", async {
        transaction
            .query_one(PostgresMetadataQueries::CONFIG_TOTALS, &[])
            .await
            .map_err(CliError::from)
    })
    .await?;
    let total_resources = unsigned_progress_total(totals.try_get(0)?, "resource count")?;
    let total_bytes = unsigned_progress_total(totals.try_get(1)?, "compressed byte count")?;
    progress.config_totals(total_resources, total_bytes);

    let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
    let rows = query_timeout("PostgreSQL Config query", async {
        transaction
            .query_raw(PostgresMetadataQueries::CONFIG, parameters)
            .await
            .map_err(CliError::from)
    })
    .await?;
    let resources = rows.map(|row| {
        let row = row?;
        Ok(ConfigResource {
            file_name: row.try_get(0)?,
            compressed: row.try_get(1)?,
        })
    });
    let (descriptors, predefined_values) = query_timeout(
        "PostgreSQL Config stream",
        decode_config_stream(
            resources,
            CONFIG_DECODE_BATCH_SIZE,
            config_pipeline_depth(),
            &mut progress,
        ),
    )
    .await?;

    progress.phase("SchemaStorage");
    let schema_rows = postgres_rows(
        transaction,
        "PostgreSQL SchemaStorage query",
        PostgresMetadataQueries::SCHEMA,
    )
    .await?;
    let schema_data: Vec<u8> = exactly_one_row(&schema_rows, "SchemaStorage")?.try_get(0)?;
    let schema = run_metadata_blocking("SchemaStorage", move || {
        parse_schema_storage(&schema_data).map_err(CliError::from)
    })
    .await?;

    progress.phase("catalog");
    let catalog_rows = postgres_rows(
        transaction,
        "PostgreSQL catalog query",
        PostgresMetadataQueries::CATALOG,
    )
    .await?;
    let live_tables = run_metadata_blocking("PostgreSQL catalog", move || {
        decode_catalog_rows(catalog_rows)
    })
    .await?;

    progress.phase("resolve");
    let resolved = run_metadata_blocking("metadata resolution", move || {
        Ok(resolve_metadata_with_predefined_values(
            db_names,
            descriptors,
            predefined_values,
            schema,
            live_tables,
        ))
    })
    .await?;
    progress.finish();
    print_resolution_report(&resolved.report);
    Ok(resolved.snapshot)
}

async fn acquire_mssql_metadata(
    client: &mut MsSqlClient<MsSqlTransport>,
) -> Result<MetadataSnapshot, CliError> {
    let mut progress = MetadataProgress::new();
    progress.phase("DBNames");
    let db_names_rows = mssql_rows(
        client,
        "MSSQL DBNames query",
        MsSqlMetadataQueries::DB_NAMES,
    )
    .await?;
    let db_names_data = required_mssql_bytes(
        exactly_one_mssql_row(&db_names_rows, "DBNames")?,
        0,
        "DBNames payload",
    )?;
    let db_names = run_metadata_blocking("DBNames", move || {
        parse_db_names(&db_names_data).map_err(CliError::from)
    })
    .await?;

    let totals = mssql_rows(
        client,
        "MSSQL Config totals query",
        MsSqlMetadataQueries::CONFIG_TOTALS,
    )
    .await?;
    let totals = exactly_one_mssql_row(&totals, "Config totals")?;
    let total_resources = unsigned_progress_total(
        required_mssql_i64(totals, 0, "Config resource count")?,
        "resource count",
    )?;
    let total_bytes = unsigned_progress_total(
        required_mssql_i64(totals, 1, "Config compressed byte count")?,
        "compressed byte count",
    )?;
    progress.config_totals(total_resources, total_bytes);

    let config_rows = query_timeout("MSSQL Config query", async {
        client
            .simple_query(MsSqlMetadataQueries::CONFIG)
            .await
            .map_err(CliError::mssql_query)
    })
    .await?
    .into_row_stream();
    let resources = config_rows.map(|row| {
        let row = row.map_err(CliError::mssql_query)?;
        Ok(ConfigResource {
            file_name: required_mssql_string(&row, 0, "Config file name")?,
            compressed: required_mssql_bytes(&row, 1, "Config payload")?,
        })
    });
    let (descriptors, predefined_values) = query_timeout(
        "MSSQL Config stream",
        decode_config_stream(
            resources,
            CONFIG_DECODE_BATCH_SIZE,
            config_pipeline_depth(),
            &mut progress,
        ),
    )
    .await?;

    progress.phase("SchemaStorage");
    let schema_rows = mssql_rows(
        client,
        "MSSQL SchemaStorage query",
        MsSqlMetadataQueries::SCHEMA,
    )
    .await?;
    let schema_data = required_mssql_bytes(
        exactly_one_mssql_row(&schema_rows, "SchemaStorage")?,
        0,
        "SchemaStorage payload",
    )?;
    let schema = run_metadata_blocking("SchemaStorage", move || {
        parse_schema_storage(&schema_data).map_err(CliError::from)
    })
    .await?;

    progress.phase("catalog");
    let catalog_rows =
        mssql_rows(client, "MSSQL catalog query", MsSqlMetadataQueries::CATALOG).await?;
    let mut catalog_values = Vec::with_capacity(catalog_rows.len());
    for row in &catalog_rows {
        catalog_values.push([
            required_mssql_string(row, 0, "catalog row tag")?,
            required_mssql_string(row, 1, "catalog table name")?,
            required_mssql_string(row, 2, "catalog value")?,
            required_mssql_string(row, 3, "catalog detail")?,
            required_mssql_string(row, 4, "catalog columns")?,
        ]);
    }
    let live_tables = run_metadata_blocking("MSSQL catalog", move || {
        decode_catalog_values(catalog_values)
    })
    .await?;

    progress.phase("resolve");
    let resolved = run_metadata_blocking("metadata resolution", move || {
        Ok(resolve_metadata_with_predefined_values(
            db_names,
            descriptors,
            predefined_values,
            schema,
            live_tables,
        ))
    })
    .await?;
    progress.finish();
    print_resolution_report(&resolved.report);
    Ok(resolved.snapshot)
}

fn print_resolution_report(report: &open_sdbl::metadata::ResolutionReport) {
    const MAX_PRINTED_FINDINGS: usize = 100;
    for finding in report.findings().iter().take(MAX_PRINTED_FINDINGS) {
        eprintln!(
            "metadata resolution: {}",
            escape_field(&finding.to_string())
        );
    }
    let omitted = report.findings().len().saturating_sub(MAX_PRINTED_FINDINGS);
    if omitted != 0 {
        eprintln!("metadata resolution: {omitted} additional findings omitted");
    }
}

async fn mssql_rows(
    client: &mut MsSqlClient<MsSqlTransport>,
    label: &str,
    sql: &str,
) -> Result<Vec<tiberius::Row>, CliError> {
    query_timeout(label, async {
        client
            .simple_query(sql)
            .await
            .map_err(CliError::mssql_query)?
            .into_first_result()
            .await
            .map_err(CliError::mssql_query)
    })
    .await
}

fn apply_mssql_cleanup(
    poisoned: &mut bool,
    rollback: Result<(), String>,
    transaction_count: Result<i32, String>,
) -> Result<(), String> {
    if let Err(error) = rollback {
        *poisoned = true;
        return Err(format!("ROLLBACK failed: {error}"));
    }
    match transaction_count {
        Ok(0) => Ok(()),
        Ok(count) => {
            *poisoned = true;
            Err(format!("@@TRANCOUNT remained {count} after ROLLBACK"))
        }
        Err(error) => {
            *poisoned = true;
            Err(format!("cannot verify @@TRANCOUNT after ROLLBACK: {error}"))
        }
    }
}

fn exactly_one_mssql_row<'rows>(
    rows: &'rows [tiberius::Row],
    name: &str,
) -> Result<&'rows tiberius::Row, CliError> {
    match rows {
        [row] => Ok(row),
        [] => Err(CliError::Data(format!("{name} resource is missing"))),
        _ => Err(CliError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

fn required_mssql_string(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<String, CliError> {
    row.try_get::<&str, _>(index)
        .map(|value| value.map(str::to_owned))
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_bytes(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<Vec<u8>, CliError> {
    row.try_get::<&[u8], _>(index)
        .map(|value| value.map(<[u8]>::to_vec))
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_i64(row: &tiberius::Row, index: usize, name: &str) -> Result<i64, CliError> {
    row.try_get::<i64, _>(index)
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_i32(row: &tiberius::Row, index: usize, name: &str) -> Result<i32, CliError> {
    row.try_get::<i32, _>(index)
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

struct ConfigResource {
    file_name: String,
    compressed: Vec<u8>,
}

struct DecodedConfigResource {
    file_name: String,
    descriptors: Vec<open_sdbl::metadata::ConfigDescriptor>,
    predefined_values: Vec<open_sdbl::metadata::ConfigPredefinedValue>,
}

async fn decode_config_stream<S>(
    resources: S,
    batch_size: usize,
    pipeline_depth: usize,
    progress: &mut MetadataProgress,
) -> Result<
    (
        Vec<open_sdbl::metadata::ConfigDescriptor>,
        Vec<open_sdbl::metadata::ConfigPredefinedValue>,
    ),
    CliError,
>
where
    S: Stream<Item = Result<ConfigResource, CliError>>,
{
    let jobs = resources
        .chunks(batch_size.max(1))
        .map(|batch| async move {
            let batch = batch.into_iter().collect::<Result<Vec<_>, _>>()?;
            tokio::task::spawn_blocking(move || {
                let resource_count = batch.len();
                let compressed_bytes = batch.iter().map(|resource| resource.compressed.len()).sum();
                let mut decoded_resources = Vec::with_capacity(resource_count);
                for resource in batch {
                    let descriptors =
                        parse_config_descriptors(&resource.file_name, &resource.compressed)?;
                    let predefined_values =
                        parse_config_predefined_values(&resource.file_name, &resource.compressed)?;
                    decoded_resources.push(DecodedConfigResource {
                        file_name: resource.file_name,
                        descriptors,
                        predefined_values,
                    });
                }
                Ok::<_, CliError>((resource_count, compressed_bytes, decoded_resources))
            })
            .await
            .map_err(|error| CliError::Data(format!("Config decoder worker failed: {error}")))?
        })
        .buffered(pipeline_depth.max(1));
    tokio::pin!(jobs);

    let mut decoded_resources = Vec::new();
    while let Some(result) = jobs.next().await {
        let (resource_count, compressed_bytes, mut batch) = result?;
        progress.advance_config(resource_count, compressed_bytes);
        decoded_resources.append(&mut batch);
    }
    decoded_resources.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    let descriptor_count = decoded_resources
        .iter()
        .map(|resource| resource.descriptors.len())
        .sum();
    let predefined_count = decoded_resources
        .iter()
        .map(|resource| resource.predefined_values.len())
        .sum();
    let mut descriptors = Vec::with_capacity(descriptor_count);
    let mut predefined_values = Vec::with_capacity(predefined_count);
    for mut resource in decoded_resources {
        descriptors.append(&mut resource.descriptors);
        predefined_values.append(&mut resource.predefined_values);
    }
    Ok((descriptors, predefined_values))
}

async fn run_metadata_blocking<T, F>(label: &'static str, work: F) -> Result<T, CliError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CliError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| CliError::Data(format!("{label} processing worker failed: {error}")))?
}

fn config_pipeline_depth() -> usize {
    std::thread::available_parallelism().map_or(4, |parallelism| {
        parallelism.get().saturating_mul(2).clamp(2, 16)
    })
}

fn unsigned_progress_total(value: i64, label: &str) -> Result<u64, CliError> {
    u64::try_from(value)
        .map_err(|_| CliError::Data(format!("database returned a negative Config {label}")))
}

async fn verify_transaction(transaction: &Transaction<'_>) -> Result<(), CliError> {
    let transaction_mode = query_timeout("PostgreSQL read-only verification", async {
        transaction
            .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
            .await
            .map_err(CliError::from)
    })
    .await?;
    let read_only: String = transaction_mode.try_get(0)?;
    let isolation: String = transaction_mode.try_get(1)?;
    if read_only != "on" || !isolation.eq_ignore_ascii_case("read committed") {
        return Err(CliError::Data(format!(
            "unsafe PostgreSQL transaction mode: read_only={read_only:?}, isolation={isolation:?}"
        )));
    }
    Ok(())
}

async fn postgres_rows(
    transaction: &Transaction<'_>,
    label: &str,
    sql: &str,
) -> Result<Vec<Row>, CliError> {
    query_timeout(label, async {
        transaction.query(sql, &[]).await.map_err(CliError::from)
    })
    .await
}

fn exactly_one_row<'rows>(rows: &'rows [Row], name: &str) -> Result<&'rows Row, CliError> {
    match rows {
        [row] => Ok(row),
        [] => Err(CliError::Data(format!("{name} resource is missing"))),
        _ => Err(CliError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

fn decode_catalog_rows(rows: Vec<Row>) -> Result<Vec<LiveTable>, CliError> {
    let values = rows
        .into_iter()
        .map(|row| {
            Ok([
                row.try_get(0)?,
                row.try_get(1)?,
                row.try_get(2)?,
                row.try_get(3)?,
                row.try_get(4)?,
            ])
        })
        .collect::<Result<Vec<_>, tokio_postgres::Error>>()?;
    decode_catalog_values(values)
}

fn decode_catalog_values(rows: Vec<[String; 5]>) -> Result<Vec<LiveTable>, CliError> {
    let mut tables = BTreeMap::<String, LiveTable>::new();
    for row in rows {
        let [tag, table_name, value, detail, columns] = row;
        let table = tables
            .entry(table_name.clone())
            .or_insert_with(|| LiveTable {
                name: table_name,
                columns: Vec::new(),
                indexes: Vec::new(),
            });
        match tag.as_str() {
            "T" => {}
            "C" => table.columns.push(LiveColumn {
                name: value,
                data_type: detail,
            }),
            "I" => table.indexes.push(LiveIndex {
                name: value,
                unique: detail == "true" || detail == "t",
                columns: columns
                    .split(',')
                    .filter(|column| !column.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }),
            _ => {
                return Err(CliError::Data(format!(
                    "unknown database catalog row tag {tag:?}"
                )));
            }
        }
    }
    Ok(tables.into_values().collect())
}

fn postgres_password(
    connection: &PostgresConnection,
    environment: &EnvironmentSecret,
) -> Result<Option<Zeroizing<String>>, CliError> {
    if let Some(password) = environment.optional("PGPASSWORD")? {
        return Ok(Some(password));
    }

    let explicit_path = env::var_os("PGPASSFILE");
    let path = explicit_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(default_password_file);
    let Some(path) = path else {
        return Ok(None);
    };
    read_password_file(&path, connection, explicit_path.is_some())
}

fn default_password_file() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".pgpass"))
}

fn read_password_file(
    path: &Path,
    connection: &PostgresConnection,
    explicit: bool,
) -> Result<Option<Zeroizing<String>>, CliError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliError::Io(
                format!("cannot open PostgreSQL password file {path:?}"),
                error,
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        CliError::Io(
            format!("cannot inspect PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    reject_insecure_password_file(path, &metadata)?;
    let mut contents = Zeroizing::new(String::new());
    file.read_to_string(&mut contents).map_err(|error| {
        CliError::Io(
            format!("cannot read PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    Ok(contents.lines().find_map(|line| {
        let record = parse_password_line(line)?;
        matches_password_field(&record.host, &connection.host)
            .then_some(())
            .filter(|_| matches_password_field(&record.port, &connection.port.to_string()))
            .filter(|_| matches_password_field(&record.database, &connection.database))
            .filter(|_| matches_password_field(&record.user, &connection.user))
            .map(|()| record.password)
    }))
}

#[cfg(unix)]
fn reject_insecure_password_file(path: &Path, metadata: &fs::Metadata) -> Result<(), CliError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !metadata.file_type().is_file() {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be a regular file"
        )));
    }
    // SAFETY: `geteuid` has no preconditions and does not retain pointers.
    let effective_uid = unsafe { libc::geteuid() };
    reject_password_file_owner(path, metadata.uid(), effective_uid)?;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must have permissions 0600 or stricter"
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn reject_password_file_owner(
    path: &Path,
    owner_uid: u32,
    effective_uid: u32,
) -> Result<(), CliError> {
    if owner_uid != effective_uid {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be owned by uid {effective_uid}, found uid {owner_uid}"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_password_file(path: &Path, metadata: &fs::Metadata) -> Result<(), CliError> {
    if metadata.file_type().is_file() {
        Ok(())
    } else {
        Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be a regular file"
        )))
    }
}

struct PasswordRecord {
    host: String,
    port: String,
    database: String,
    user: String,
    password: Zeroizing<String>,
}

fn parse_password_line(line: &str) -> Option<PasswordRecord> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = Vec::with_capacity(5);
    let mut field = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            field.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' && fields.len() < 4 {
            fields.push(std::mem::take(&mut field));
        } else {
            field.push(character);
        }
    }
    if escaped {
        field.push('\\');
    }
    fields.push(field);
    let [host, port, database, user, password] = fields.try_into().ok()?;
    Some(PasswordRecord {
        host,
        port,
        database,
        user,
        password: Zeroizing::new(password),
    })
}

fn matches_password_field(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

fn print_snapshot(output: &mut impl Write, snapshot: &MetadataSnapshot) -> io::Result<()> {
    writeln!(
        output,
        "RECORD\tGUID\tKIND\tNAME\tPHYSICAL_NAME\tOWNER\tSCHEMA\tLIVE\tDETAIL"
    )?;
    let total_rows = snapshot.objects.len() + snapshot.fields.len() + snapshot.indexes.len();
    let mut printed_rows = 0;
    for object in snapshot.objects.iter().take(MAX_PRINTED_ROWS) {
        let mut details = Vec::new();
        if let Some(allowed_length) = object.code_allowed_length {
            details.push(format!("Code={}", allowed_length.as_str()));
        }
        if let Some(allowed_length) = object.number_allowed_length {
            details.push(format!("Number={}", allowed_length.as_str()));
        }
        writeln!(
            output,
            "OBJECT\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}",
            object.guid,
            object.kind.map_or("NonTabular", |kind| kind.as_str()),
            bounded_field(object.name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(
                object.physical_table.as_deref().unwrap_or(""),
                MAX_CELL_WIDTH
            ),
            yes_no(object.declared),
            yes_no(object.live),
            bounded_field(&details.join(","), MAX_CELL_WIDTH),
        )?;
        printed_rows += 1;
    }
    for field in snapshot
        .fields
        .iter()
        .take(MAX_PRINTED_ROWS.saturating_sub(printed_rows))
    {
        writeln!(
            output,
            "FIELD\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            field.guid,
            if field.data_separator {
                "DataSeparator"
            } else {
                "Field"
            },
            bounded_field(field.name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(&field.physical_name, MAX_CELL_WIDTH),
            bounded_field(&field.owner_tables.join(","), MAX_CELL_WIDTH),
            yes_no(field.declared),
            yes_no(field.live),
        )?;
        printed_rows += 1;
    }
    for index in snapshot
        .indexes
        .iter()
        .take(MAX_PRINTED_ROWS.saturating_sub(printed_rows))
    {
        writeln!(
            output,
            "INDEX\t\tIndex\t{}\t{}\t{}\tyes\t{}\t{}",
            bounded_field(&index.declared_name, MAX_CELL_WIDTH),
            bounded_field(index.live_name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(&index.table, MAX_CELL_WIDTH),
            yes_no(index.live_name.is_some() && index.unique_matches),
            bounded_field(&index.logical_key.join(","), MAX_CELL_WIDTH),
        )?;
        printed_rows += 1;
    }
    let omitted = total_rows.saturating_sub(printed_rows);
    if omitted != 0 {
        writeln!(output, "# {omitted} rows omitted")?;
    }
    Ok(())
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn escape_field(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            value
                if value.is_control()
                    || matches!(
                        value,
                        '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                    ) =>
            {
                write!(&mut escaped, "\\u{{{:x}}}", u32::from(value))
                    .expect("writing to a String cannot fail");
            }
            value => escaped.push(value),
        }
    }
    escaped
}

fn bounded_field(value: &str, max_width: usize) -> String {
    let mut escaped = escape_field(value);
    if UnicodeWidthStr::width(escaped.as_str()) <= max_width {
        return escaped;
    }
    let ellipsis_width = UnicodeWidthChar::width('…').unwrap_or(1);
    let content_width = max_width.saturating_sub(ellipsis_width);
    let mut width = 0;
    let mut end = 0;
    for (offset, character) in escaped.char_indices() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > content_width {
            break;
        }
        width += character_width;
        end = offset + character.len_utf8();
    }
    escaped.truncate(end);
    if max_width >= ellipsis_width {
        escaped.push('…');
    }
    escaped
}

fn read_lex_source(path: &str) -> Result<String, CliError> {
    if path == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?;
        Ok(source)
    } else {
        fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path:?}"), error))
    }
}

fn lex(output: &mut impl Write, tokens: &[open_sdbl::Token<'_>]) -> io::Result<()> {
    for token in tokens.iter().take(MAX_PRINTED_ROWS) {
        writeln!(
            output,
            "{}:{}\t{}\t{}",
            token.span.line,
            token.span.column,
            token.kind,
            bounded_field(token.lexeme, MAX_CELL_WIDTH)
        )?;
    }
    let omitted = tokens.len().saturating_sub(MAX_PRINTED_ROWS);
    if omitted != 0 {
        writeln!(output, "# {omitted} rows omitted")?;
    }
    Ok(())
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
    Metadata(MetadataError),
    Data(String),
    Database(String),
    MsSql {
        operation: &'static str,
        source: tiberius::error::Error,
    },
    Terminal(String),
}

impl CliError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) | Self::Metadata(_) | Self::Data(_) => 1,
            Self::Usage(_)
            | Self::Io(_, _)
            | Self::Database(_)
            | Self::MsSql { .. }
            | Self::Terminal(_) => 2,
        }
    }

    fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL connection failed: {error}"))
    }

    fn mssql_connection(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "connection",
            source: error,
        }
    }

    fn mssql_query(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "query",
            source: error,
        }
    }

    fn socks5_connection(error: impl fmt::Display) -> Self {
        Self::Database(format!("SOCKS5 proxy connection failed: {error}"))
    }

    fn standard_output(error: io::Error) -> Self {
        Self::Io("cannot write standard output".to_owned(), error)
    }

    fn is_broken_pipe(&self) -> bool {
        matches!(self, Self::Io(_, error) if error.kind() == io::ErrorKind::BrokenPipe)
    }

    fn is_mssql_connection_failure(&self) -> bool {
        matches!(
            self,
            Self::MsSql {
                source: tiberius::error::Error::Io { .. }
                    | tiberius::error::Error::Protocol(_)
                    | tiberius::error::Error::Tls(_),
                ..
            }
        )
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
            Self::MsSql { operation, source } => {
                write!(formatter, "MSSQL {operation} failed: {source}")
            }
            Self::Data(message) | Self::Database(message) | Self::Terminal(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl From<MetadataError> for CliError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<tokio_postgres::Error> for CliError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL query failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::hex_test_support::hex;
    #[cfg(unix)]
    use super::reject_password_file_owner;
    use super::{
        CliError, ConfigResource, ConnectionOptions, Credentials, DatabaseConnection,
        EnvironmentSecret, INSECURE_MSSQL_CERTIFICATE_WARNING, MAX_CELL_WIDTH, MAX_PRINTED_ROWS,
        MSSQL_VERIFY_READONLY, MetadataProgress, MsSqlConnection, MsSqlSession, PostgresConnection,
        PostgresSslMode, Socks5Proxy, Zeroizing, apply_mssql_cleanup, await_postgres_driver,
        bounded_database_call, bounded_field, connect_postgres_raw, connect_socks5,
        decode_catalog_values, decode_config_stream, escape_field, format_mssql_binary, lex,
        parse_connection, parse_password_line, parse_socks5_proxy, read_password_file,
        reject_insecure_password_file, render_metadata_progress, select_postgres_sslmode,
        socks5_connect_request,
    };
    use open_sdbl::metadata::{FieldId, MetadataSnapshot, StandardFieldId};
    use open_sdbl::query::{
        CompiledQuery, MsSqlBackend, PresentationExpression, PresentationPlan, QueryCompiler,
    };

    fn compile_mssql_test_query(
        source: &str,
        snapshot: &MetadataSnapshot,
        year_offset: i32,
    ) -> CompiledQuery {
        let backend = MsSqlBackend::new(year_offset).expect("test MSSQL year offset must be valid");
        let prepared = QueryCompiler::new(snapshot, backend)
            .prepare(source)
            .unwrap();
        let plans = prepared
            .presentation_request()
            .targets
            .iter()
            .map(|target| {
                let description = FieldId::Standard(StandardFieldId::Description);
                let code = FieldId::Standard(StandardFieldId::Code);
                PresentationPlan {
                    object: target.object,
                    fields: vec![description, code],
                    expression: PresentationExpression::Concat(vec![
                        PresentationExpression::Field(description),
                        PresentationExpression::Literal(" (".to_owned()),
                        PresentationExpression::Field(code),
                        PresentationExpression::Literal(")".to_owned()),
                    ]),
                }
            })
            .collect::<Vec<_>>();
        prepared.compile(snapshot, &plans).unwrap()
    }

    fn mssql_test_connection() -> MsSqlConnection {
        let user = std::env::var("OPEN_SDBL_MSSQL_TEST_USER")
            .expect("OPEN_SDBL_MSSQL_TEST_USER must name a SELECT-only SQL login");
        MsSqlConnection {
            options: ConnectionOptions {
                host: std::env::var("OPEN_SDBL_MSSQL_TEST_HOST")
                    .unwrap_or_else(|_| "192.168.122.222".to_owned()),
                port: std::env::var("OPEN_SDBL_MSSQL_TEST_PORT")
                    .map_or(1433, |value| value.parse().expect("invalid test port")),
                database: std::env::var("OPEN_SDBL_MSSQL_TEST_DATABASE")
                    .unwrap_or_else(|_| "demo".to_owned()),
                user,
                socks5_proxy: None,
            },
            trust_server_certificate: std::env::var_os("OPEN_SDBL_MSSQL_TEST_TRUST_CERTIFICATE")
                .is_some(),
            trust_ca_file: None,
        }
    }

    fn mssql_test_credentials() -> Credentials {
        Credentials {
            postgres: EnvironmentSecret::Missing,
            mssql: EnvironmentSecret::Present(Zeroizing::new(
                std::env::var("MSSQL_PASSWORD").expect("MSSQL_PASSWORD is required"),
            )),
            socks5: EnvironmentSecret::Missing,
        }
    }

    #[tokio::test]
    #[ignore = "requires OPEN_SDBL_MSSQL_TEST_USER, MSSQL_PASSWORD, and a live 1C database"]
    async fn reads_metadata_from_the_mssql_demo_database() {
        let mut session =
            MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
                .await
                .unwrap();
        let snapshot = session.metadata().await.unwrap();
        assert!(!snapshot.objects.is_empty());
        assert!(!snapshot.live_tables.is_empty());
        session.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires the MSSQL demo database and its _ДемоЗаказПокупателя document"]
    async fn reads_native_rowversion_from_the_mssql_demo_database() {
        let mut session =
            MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
                .await
                .unwrap();
        let backend = session.backend;
        let snapshot = session.metadata().await.unwrap();
        let compiled = QueryCompiler::new(&snapshot, backend)
            .compile(
                "SELECT Version FROM Документ._ДемоЗаказПокупателя WHERE Version > 0x00000000000007D6;",
            )
            .unwrap();

        assert!(
            compiled
                .sql
                .contains("\"__src\".\"_Version\" AS \"Version\"")
        );
        assert!(
            !compiled
                .sql
                .contains("CONVERT(nvarchar(max), \"__src\".\"_Version\")")
        );
        assert!(
            compiled
                .sql
                .contains("(\"__src\".\"_Version\" > 0x00000000000007D6)")
        );
        let rows = session
            .query(&compiled.sql, compiled.columns.len())
            .await
            .unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            let version = row[0].as_deref().unwrap();
            assert!(version > "0x00000000000007D6");
            assert_eq!(version.len(), 18);
        }
        session.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires the MSSQL demo database and its _Reference18X1 extension table"]
    async fn reads_dereferences_and_presents_the_mssql_demo_extension_table() {
        let mut session =
            MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
                .await
                .unwrap();
        let snapshot = session.metadata().await.unwrap();
        let direct = compile_mssql_test_query(
            "SELECT TOP 3 ID, Code, Description FROM Catalog._ДемоНоменклатура;",
            &snapshot,
            session.backend.year_offset(),
        );
        assert!(direct.sql.contains("FROM \"_Reference18X1\""));
        let direct_rows = session
            .query(&direct.sql, direct.columns.len())
            .await
            .unwrap();
        assert_eq!(direct_rows.len(), 3);
        assert!(direct_rows.iter().all(|row| {
            row[2]
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty())
        }));

        let dereference = compile_mssql_test_query(
            "SELECT TOP 3 Номенклатура.Наименование FROM РегистрНакопления._ДемоОстаткиТоваровВМестахХранения.Остатки();",
            &snapshot,
            session.backend.year_offset(),
        );
        assert!(dereference.sql.contains("FROM \"_Reference18X1\""));
        let dereference_rows = session
            .query(&dereference.sql, dereference.columns.len())
            .await
            .unwrap();
        assert_eq!(dereference_rows.len(), 3);
        assert!(dereference_rows.iter().all(|row| {
            row[0]
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty())
        }));

        let presentations = compile_mssql_test_query(
            "SELECT Номенклатура, ПредставлениеСсылки(Номенклатура), Представление(Номенклатура), КоличествоОстаток FROM РегистрНакопления._ДемоОстаткиТоваровВМестахХранения.Остатки();",
            &snapshot,
            session.backend.year_offset(),
        );
        assert!(presentations.sql.contains("FROM \"_Reference18X1\""));
        let presentation_rows = session
            .query(&presentations.sql, presentations.columns.len())
            .await
            .unwrap();
        assert!(!presentation_rows.is_empty());
        assert!(presentation_rows.iter().all(|row| {
            let reference = row[1].as_deref();
            let value = row[2].as_deref();
            reference == value
                && reference.is_some_and(|presentation| {
                    !presentation.trim().is_empty() && presentation != " ()"
                })
        }));
        session.close().await.unwrap();
    }

    #[test]
    fn renders_mssql_binary_as_hexadecimal_text() {
        assert_eq!(format_mssql_binary(&[0x00, 0x7d, 0xd6]), "0x007DD6");
    }

    #[test]
    fn escapes_terminal_controls_and_bidirectional_overrides_in_one_pass() {
        assert_eq!(escape_field("\\\t\r\n"), "\\\\\\t\\r\\n");
        assert_eq!(escape_field("\x1b[2J"), "\\u{1b}[2J");
        assert_eq!(
            escape_field("\x1b]52;c;payload\x07"),
            "\\u{1b}]52;c;payload\\u{7}"
        );
        assert_eq!(
            escape_field("a\u{2028}\u{2029}\u{202e}b\u{2066}c\u{2069}"),
            "a\\u{2028}\\u{2029}\\u{202e}b\\u{2066}c\\u{2069}"
        );
    }

    #[test]
    fn bounds_lex_rows_and_lexeme_width() {
        let source = std::iter::repeat_n("x", MAX_PRINTED_ROWS + 1)
            .collect::<Vec<_>>()
            .join(" ");
        let tokens = open_sdbl::tokenize(&source).unwrap();
        let mut output = Vec::new();
        lex(&mut output, &tokens).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.ends_with("# 1 rows omitted\n"));

        let long = "界".repeat(MAX_CELL_WIDTH);
        let bounded = bounded_field(&long, MAX_CELL_WIDTH);
        assert!(unicode_width::UnicodeWidthStr::width(bounded.as_str()) <= MAX_CELL_WIDTH);
        assert!(bounded.ends_with('…'));
    }

    #[test]
    fn selects_secure_postgres_ssl_modes_with_flag_precedence() {
        assert_eq!(
            select_postgres_sslmode(None, None).unwrap(),
            PostgresSslMode::VerifyFull
        );
        assert_eq!(
            select_postgres_sslmode(None, Some("verify-ca")).unwrap(),
            PostgresSslMode::VerifyCa
        );
        assert_eq!(
            select_postgres_sslmode(Some(PostgresSslMode::Require), Some("invalid")).unwrap(),
            PostgresSslMode::Require
        );
        assert!(select_postgres_sslmode(None, Some("prefer")).is_err());
    }

    #[test]
    fn failed_mssql_cleanup_poisons_the_session_state() {
        let mut poisoned = false;
        let error = apply_mssql_cleanup(&mut poisoned, Err("connection lost".to_owned()), Ok(0))
            .unwrap_err();
        assert!(poisoned);
        assert!(error.contains("ROLLBACK failed"));

        let mut poisoned = false;
        let error = apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(1)).unwrap_err();
        assert!(poisoned);
        assert!(error.contains("@@TRANCOUNT remained 1"));

        let mut poisoned = false;
        apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(0)).unwrap();
        assert!(!poisoned);
        assert!(MSSQL_VERIFY_READONLY.contains("db_datareader"));
        assert!(MSSQL_VERIFY_READONLY.contains("@@TRANCOUNT"));
    }

    #[tokio::test]
    async fn times_out_a_stalled_post_handshake_server_call() {
        use tokio::io::AsyncReadExt;
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let mut stream = TcpStream::connect(address).await.unwrap();
        let error =
            bounded_database_call("fake database query", Duration::from_millis(20), async {
                let mut byte = [0_u8; 1];
                stream
                    .read_exact(&mut byte)
                    .await
                    .map_err(|error| CliError::Io("fake query read".to_owned(), error))?;
                Ok(())
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn aborts_a_stalled_postgres_driver_on_close() {
        let driver = tokio::spawn(async {
            std::future::pending::<Result<(), tokio_postgres::Error>>().await
        });
        let error = await_postgres_driver(driver, Duration::from_millis(20))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("driver aborted"));
    }

    #[test]
    fn refuses_postgres_plaintext_without_explicit_opt_in() {
        let base = [
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--sslmode",
            "disable",
        ];
        let mut refused = base.into_iter().map(str::to_owned);
        let error = parse_connection(&mut refused, "console", &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("requires --insecure-plaintext"));

        let mut accepted = base
            .into_iter()
            .chain(["--insecure-plaintext"])
            .map(str::to_owned);
        let connection = parse_connection(&mut accepted, "console", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::Postgres(connection) = connection else {
            panic!("expected PostgreSQL connection");
        };
        assert_eq!(connection.sslmode, PostgresSslMode::Disable);
    }

    #[test]
    fn parses_mssql_provider_defaults_and_explicit_tls_exception() {
        let mut arguments = [
            "mssql",
            "--host",
            "192.168.122.222",
            "--database",
            "demo",
            "--user",
            "reader",
            "--trust-server-certificate",
        ]
        .into_iter()
        .map(str::to_owned);
        let connection = parse_connection(&mut arguments, "metadata", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::MsSql(connection) = connection else {
            panic!("expected MSSQL connection");
        };
        assert_eq!(connection.options.host, "192.168.122.222");
        assert_eq!(connection.options.port, 1433);
        assert_eq!(connection.options.database, "demo");
        assert!(connection.trust_server_certificate);
        assert!(INSECURE_MSSQL_CERTIFICATE_WARNING.contains("disables MSSQL certificate"));
    }

    #[test]
    fn parses_mssql_ca_file_and_rejects_it_for_postgres() {
        let mut arguments = [
            "mssql",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--trust-ca-file",
            "company-ca.pem",
        ]
        .into_iter()
        .map(str::to_owned);
        let connection = parse_connection(&mut arguments, "metadata", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::MsSql(connection) = connection else {
            panic!("expected MSSQL connection");
        };
        assert_eq!(connection.trust_ca_file.as_deref(), Some("company-ca.pem"));

        let mut arguments = [
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--trust-ca-file",
            "company-ca.pem",
        ]
        .into_iter()
        .map(str::to_owned);
        let error = parse_connection(&mut arguments, "metadata", &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("unknown metadata option"));
    }

    #[test]
    fn rejects_mssql_only_tls_flag_for_postgres() {
        let mut arguments = [
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--trust-server-certificate",
        ]
        .into_iter()
        .map(str::to_owned);
        let error = parse_connection(&mut arguments, "console", &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("unknown console option"));
    }

    #[test]
    fn decodes_provider_neutral_catalog_rows() {
        let tables = decode_catalog_values(vec![
            ["T", "_Reference1", "", "", ""].map(str::to_owned),
            ["C", "_Reference1", "_IDRRef", "binary(16)", ""].map(str::to_owned),
            ["I", "_Reference1", "_Reference1_PK", "true", "_IDRRef"].map(str::to_owned),
        ])
        .unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].columns[0].data_type, "binary(16)");
        assert!(tables[0].indexes[0].unique);
    }

    #[test]
    fn renders_metadata_progress_with_exact_resource_and_byte_totals() {
        assert_eq!(
            render_metadata_progress("Config", 25, 100, 512 * 1024, 1024 * 1024, 10),
            "metadata [#####-----]  50.0% Config 25/100 512.0 KiB/1.0 MiB"
        );
    }

    #[tokio::test]
    async fn streamed_config_decoding_preserves_order_and_propagates_errors() {
        let compressed = hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        );
        let resources = futures_util::stream::iter([
            Ok(ConfigResource {
                file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
                compressed: compressed.clone(),
            }),
            Ok(ConfigResource {
                file_name: "25c96bd3-fac4-42ef-b695-74c9af43589b".to_owned(),
                compressed: compressed.clone(),
            }),
        ]);
        let mut progress = MetadataProgress::disabled();
        progress.config_totals(2, (compressed.len() * 2) as u64);
        let (descriptors, predefined_values) = decode_config_stream(resources, 2, 2, &mut progress)
            .await
            .unwrap();
        assert!(!descriptors.is_empty());
        assert!(predefined_values.is_empty());
        assert_eq!(
            descriptors.first().unwrap().resource_guid.as_str(),
            "25c96bd3-fac4-42ef-b695-74c9af43589b"
        );
        assert_eq!(
            descriptors.last().unwrap().resource_guid.as_str(),
            "b8bac76b-c91b-4d78-8a70-ffa39f8de694"
        );
        assert_eq!(progress.completed_resources, 2);
        assert_eq!(progress.completed_bytes, (compressed.len() * 2) as u64);

        let invalid = futures_util::stream::iter([Ok(ConfigResource {
            file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
            compressed: b"not deflate".to_vec(),
        })]);
        let error = decode_config_stream(invalid, 2, 2, &mut MetadataProgress::disabled())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("DEFLATE"));
    }

    #[test]
    fn parses_socks5_proxy_endpoints() {
        assert_eq!(
            parse_socks5_proxy("proxy.example:1080").unwrap(),
            Socks5Proxy {
                host: "proxy.example".to_owned(),
                port: 1080,
                username: None,
            }
        );
        assert_eq!(
            parse_socks5_proxy("[2001:db8::1]:9050").unwrap(),
            Socks5Proxy {
                host: "2001:db8::1".to_owned(),
                port: 9050,
                username: None,
            }
        );
        assert!(parse_socks5_proxy("proxy.example").is_err());
        assert!(parse_socks5_proxy("2001:db8::1:1080").is_err());
        assert!(parse_socks5_proxy(":1080").is_err());
        assert!(parse_socks5_proxy("proxy.example:0").is_err());

        let mut arguments = [
            "mssql",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--socks5-user",
            "proxy-reader",
            "--socks5-proxy",
            "proxy.example:1080",
        ]
        .into_iter()
        .map(str::to_owned);
        let connection = parse_connection(&mut arguments, "console", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::MsSql(connection) = connection else {
            panic!("expected MSSQL connection");
        };
        assert_eq!(
            connection
                .options
                .socks5_proxy
                .as_ref()
                .and_then(|proxy| proxy.username.as_deref()),
            Some("proxy-reader")
        );
    }

    #[test]
    fn encodes_ip_targets_in_socks5_connect_requests() {
        assert_eq!(
            socks5_connect_request("192.0.2.1", 5432).unwrap(),
            vec![0x05, 0x01, 0x00, 0x01, 192, 0, 2, 1, 0x15, 0x38]
        );
        assert_eq!(
            socks5_connect_request("2001:db8::1", 15432).unwrap(),
            vec![
                0x05, 0x01, 0x00, 0x04, 0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                0x3c, 0x48,
            ]
        );
    }

    #[tokio::test]
    async fn sends_database_hostname_to_socks5_proxy() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x00]);
            stream.write_all(&[0x05, 0x00]).await.unwrap();

            let mut request = [0_u8; 5];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request[..4], &[0x05, 0x01, 0x00, 0x03]);
            let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
            stream.read_exact(&mut host_and_port).await.unwrap();
            assert_eq!(
                &host_and_port[..host_and_port.len() - 2],
                b"database.internal"
            );
            assert_eq!(
                &host_and_port[host_and_port.len() - 2..],
                &15432_u16.to_be_bytes()
            );
            stream
                .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x12, 0x34])
                .await
                .unwrap();
        });

        let proxy = Socks5Proxy {
            host: address.ip().to_string(),
            port: address.port(),
            username: None,
        };
        let stream = connect_socks5(&proxy, None, "database.internal", 15432)
            .await
            .unwrap();
        drop(stream);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn reports_missing_socks5_authentication_credentials() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            stream.write_all(&[0x05, 0x02]).await.unwrap();
        });
        let proxy = Socks5Proxy {
            host: address.ip().to_string(),
            port: address.port(),
            username: None,
        };

        let error = connect_socks5(&proxy, None, "database.internal", 5432)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("requires SOCKS5 username/password authentication"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn authenticates_to_socks5_with_username_and_password() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0_u8; 4];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x02, 0x00, 0x02]);
            stream.write_all(&[0x05, 0x02]).await.unwrap();

            let mut authentication = [0_u8; 13];
            stream.read_exact(&mut authentication).await.unwrap();
            assert_eq!(&authentication, b"\x01\x04user\x06secret");
            stream.write_all(&[0x01, 0x00]).await.unwrap();

            let mut request = [0_u8; 5];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request[..4], &[0x05, 0x01, 0x00, 0x03]);
            let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
            stream.read_exact(&mut host_and_port).await.unwrap();
            assert_eq!(
                &host_and_port[..host_and_port.len() - 2],
                b"database.internal"
            );
            stream
                .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x12, 0x34])
                .await
                .unwrap();
        });
        let proxy = Socks5Proxy {
            host: address.ip().to_string(),
            port: address.port(),
            username: Some("user".to_owned()),
        };

        let stream = connect_socks5(&proxy, Some("secret"), "database.internal", 5432)
            .await
            .unwrap();
        drop(stream);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn reports_socks5_reply_before_a_malformed_reserved_byte() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            stream.write_all(&[0x05, 0x00]).await.unwrap();
            let mut request = [0_u8; 5];
            stream.read_exact(&mut request).await.unwrap();
            let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
            stream.read_exact(&mut host_and_port).await.unwrap();
            stream.write_all(&[0x05, 0x05, 0x01, 0x01]).await.unwrap();
        });
        let proxy = Socks5Proxy {
            host: address.ip().to_string(),
            port: address.port(),
            username: None,
        };

        let error = connect_socks5(&proxy, None, "database.internal", 5432)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("connection refused"), "{error}");
        assert!(!error.contains("malformed"), "{error}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn times_out_silent_postgres_startup_through_socks5() {
        use std::time::Duration;

        use tokio::io::AsyncReadExt;
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut startup = [0_u8; 1024];
            assert!(stream.read(&mut startup).await.unwrap() > 0);
            assert_eq!(stream.read(&mut startup).await.unwrap(), 0);
        });
        let stream = TcpStream::connect(address).await.unwrap();
        let mut configuration = tokio_postgres::Config::new();
        configuration.user("reader").dbname("test");

        let error =
            match connect_postgres_raw(&configuration, stream, Duration::from_millis(25)).await {
                Ok(_) => panic!("silent PostgreSQL startup unexpectedly succeeded"),
                Err(error) => error,
            };
        assert_eq!(
            error.to_string(),
            "PostgreSQL startup through SOCKS5 timed out after 25ms"
        );
        server.await.unwrap();
    }

    #[test]
    fn parses_password_file_escaping_and_wildcards() {
        let record = parse_password_line(r"host\:part:5432:*:reader:pa\\ss\:word").unwrap();
        assert_eq!(record.host, "host:part");
        assert_eq!(record.database, "*");
        assert_eq!(record.password.as_str(), r"pa\ss:word");
        assert!(parse_password_line("# comment").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn reads_the_first_matching_secure_password_record() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "open-sdbl-pgpass-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(
            &path,
            "other:5432:test:reader:wrong\n*:5432:test:reader:secret\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(&path, permissions).unwrap();
        let connection = PostgresConnection {
            options: ConnectionOptions {
                host: "db".to_owned(),
                port: 5432,
                database: "test".to_owned(),
                user: "reader".to_owned(),
                socks5_proxy: None,
            },
            sslmode: PostgresSslMode::VerifyFull,
        };

        let password = read_password_file(&path, &connection, true)
            .unwrap()
            .unwrap();
        assert_eq!(password.as_str(), "secret");
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_regular_and_wrong_owner_password_files() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::MetadataExt;

        let path = std::env::temp_dir().join(format!(
            "open-sdbl-pgpass-fifo-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path_bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: `path_bytes` is a valid NUL-terminated path and the mode is valid.
        assert_eq!(unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) }, 0);
        let metadata = std::fs::metadata(&path).unwrap();
        let error = reject_insecure_password_file(&path, &metadata).unwrap_err();
        assert!(error.to_string().contains("regular file"));
        std::fs::remove_file(&path).unwrap();

        let owner = metadata.uid();
        let error = reject_password_file_owner(&path, owner, owner.wrapping_add(1)).unwrap_err();
        assert!(error.to_string().contains("must be owned by uid"));
    }

    #[test]
    fn rejects_password_bearing_command_line_flags() {
        for option in ["--password", "--db-password", "--socks5-password"] {
            let mut arguments = [
                "postgres",
                "--host",
                "db",
                "--database",
                "test",
                "--user",
                "reader",
                option,
                "secret",
            ]
            .into_iter()
            .map(str::to_owned);
            let error = parse_connection(&mut arguments, "console", &mut Vec::new()).unwrap_err();
            assert!(error.to_string().contains("unknown console option"));
        }
    }
}
