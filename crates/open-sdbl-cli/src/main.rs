use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use futures_util::{Stream, StreamExt};
use open_sdbl::metadata::{
    LiveColumn, LiveIndex, LiveTable, MetadataError, MetadataSnapshot, MsSqlMetadataQueries,
    PostgresMetadataQueries, parse_config_descriptors, parse_config_predefined_values,
    parse_db_names, parse_schema_storage, resolve_metadata_with_predefined_values,
};
use open_sdbl::{Diagnostic, tokenize};
use tiberius::{
    AuthMethod, Client as MsSqlClient, ColumnType as MsSqlColumnType, Config as MsSqlConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_postgres::types::ToSql;
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

mod repl;

const HELP: &str = "open-sdbl — tooling for the 1C query language\n\n\
Usage:\n  open-sdbl lex [FILE|-]\n  open-sdbl metadata postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl metadata mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl --help\n\n\
Commands:\n  lex       Print lexical tokens; reads standard input when FILE is '-' or omitted\n  metadata  Read and resolve 1C information-base metadata\n  console   Run 1C queries and inspect resolved metadata interactively\n\n\
PostgreSQL options:\n  --port PORT                 PostgreSQL port (default: 5432)\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy (no authentication)\n\n\
MSSQL options:\n  --port PORT                 SQL Server port (default: 1433)\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy (no authentication)\n  --trust-server-certificate  Accept an untrusted TLS certificate (development only)\n\n\
Authentication:\n  PostgreSQL: PGPASSWORD, PGPASSFILE, or $HOME/.pgpass\n  MSSQL: MSSQL_PASSWORD\n";

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_REDRAW_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_BAR_WIDTH: usize = 24;
const CONFIG_DECODE_BATCH_SIZE: usize = 256;

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

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run() -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print!("{HELP}");
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" => {
            print!("{HELP}");
            Ok(())
        }
        "lex" => {
            let path = arguments.next().unwrap_or_else(|| "-".to_owned());
            if let Some(unexpected) = arguments.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument {unexpected:?}\n\n{HELP}"
                )));
            }
            lex(&path)
        }
        "metadata" => metadata(arguments).await,
        "console" | "repl" => console(arguments).await,
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

async fn metadata(mut arguments: impl Iterator<Item = String>) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "metadata")? else {
        return Ok(());
    };

    let mut session = DatabaseSession::connect(&connection).await?;
    let result = session.metadata().await;
    let close_result = session.close().await;
    let snapshot = result?;
    close_result?;
    print_snapshot(snapshot);
    Ok(())
}

async fn console(mut arguments: impl Iterator<Item = String>) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "console")? else {
        return Ok(());
    };
    let mut session = DatabaseSession::connect(&connection).await?;
    let result = async {
        let snapshot = session.metadata().await?;
        repl::run(&mut session, snapshot).await
    }
    .await;
    let close_result = session.close().await;
    result?;
    close_result
}

fn parse_connection(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
) -> Result<Option<DatabaseConnection>, CliError> {
    let Some(provider) = arguments.next() else {
        return Err(CliError::Usage(format!(
            "missing {command} provider\n\n{HELP}"
        )));
    };
    if matches!(provider.as_str(), "-h" | "--help") {
        print!("{HELP}");
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
    while let Some(option) = arguments.next() {
        if matches!(option.as_str(), "-h" | "--help") {
            print!("{HELP}");
            return Ok(None);
        }
        if option == "--trust-server-certificate" {
            if provider != "mssql" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
            trust_server_certificate = true;
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

    Ok(Some(if provider == "postgres" {
        DatabaseConnection::Postgres(PostgresConnection(options))
    } else {
        DatabaseConnection::MsSql(MsSqlConnection {
            options,
            trust_server_certificate,
        })
    }))
}

#[derive(Debug)]
enum DatabaseConnection {
    Postgres(PostgresConnection),
    MsSql(MsSqlConnection),
}

#[derive(Debug)]
struct ConnectionOptions {
    host: String,
    port: u16,
    database: String,
    user: String,
    socks5_proxy: Option<Socks5Proxy>,
}

#[derive(Debug)]
struct PostgresConnection(ConnectionOptions);

impl std::ops::Deref for PostgresConnection {
    type Target = ConnectionOptions;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
struct MsSqlConnection {
    options: ConnectionOptions,
    trust_server_certificate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Socks5Proxy {
    host: String,
    port: u16,
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
    })
}

struct PostgresSession {
    client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
}

impl PostgresSession {
    async fn connect(connection: &PostgresConnection) -> Result<Self, CliError> {
        let mut configuration = tokio_postgres::Config::new();
        configuration
            .host(&connection.host)
            .port(connection.port)
            .dbname(&connection.database)
            .user(&connection.user)
            .connect_timeout(CONNECTION_TIMEOUT);
        if let Some(password) = postgres_password(connection)? {
            configuration.password(password);
        }

        let (client, driver) = if let Some(proxy) = &connection.socks5_proxy {
            let stream = connect_socks5(proxy, &connection.host, connection.port).await?;
            connect_postgres_raw(&configuration, stream, CONNECTION_TIMEOUT).await?
        } else {
            let (client, connection_driver) = configuration
                .connect(NoTls)
                .await
                .map_err(CliError::database_connection)?;
            (client, tokio::spawn(connection_driver))
        };
        Ok(Self { client, driver })
    }

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        let transaction = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .read_only(true)
            .start()
            .await?;
        let snapshot = acquire_metadata(&transaction).await;
        match snapshot {
            Ok(snapshot) => {
                transaction.commit().await?;
                Ok(snapshot)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, CliError> {
        let transaction = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .read_only(true)
            .start()
            .await?;
        if let Err(error) = verify_transaction(&transaction).await {
            let _ = transaction.rollback().await;
            return Err(error);
        }
        match transaction.query(sql, &[]).await {
            Ok(rows) => {
                let rows = rows
                    .iter()
                    .map(|row| {
                        (0..column_count)
                            .map(|index| row.try_get(index).map_err(CliError::from))
                            .collect()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                transaction.commit().await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error.into())
            }
        }
    }

    async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        self.driver
            .await
            .map_err(|error| {
                CliError::Database(format!("PostgreSQL connection task failed: {error}"))
            })?
            .map_err(CliError::database_connection)
    }
}

pub(crate) type QueryRows = Vec<Vec<Option<String>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatabaseDialect {
    Postgres,
    MsSql { year_offset: i32 },
}

enum DatabaseSession {
    Postgres(PostgresSession),
    MsSql(Box<MsSqlSession>),
}

impl DatabaseSession {
    async fn connect(connection: &DatabaseConnection) -> Result<Self, CliError> {
        match connection {
            DatabaseConnection::Postgres(connection) => PostgresSession::connect(connection)
                .await
                .map(Self::Postgres),
            DatabaseConnection::MsSql(connection) => MsSqlSession::connect(connection)
                .await
                .map(Box::new)
                .map(Self::MsSql),
        }
    }

    pub(crate) const fn dialect(&self) -> DatabaseDialect {
        match self {
            Self::Postgres(_) => DatabaseDialect::Postgres,
            Self::MsSql(session) => DatabaseDialect::MsSql {
                year_offset: session.year_offset,
            },
        }
    }

    pub(crate) const fn execution_label(&self) -> &'static str {
        match self {
            Self::Postgres(_) => "PostgreSQL execution",
            Self::MsSql(_) => "MSSQL execution",
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

struct MsSqlSession {
    client: MsSqlClient<MsSqlTransport>,
    database: String,
    year_offset: i32,
}

impl MsSqlSession {
    async fn connect(connection: &MsSqlConnection) -> Result<Self, CliError> {
        let options = &connection.options;
        let password = env::var("MSSQL_PASSWORD").map_err(|error| match error {
            env::VarError::NotPresent => CliError::Data(
                "MSSQL_PASSWORD is required for SQL Server authentication".to_owned(),
            ),
            env::VarError::NotUnicode(_) => {
                CliError::Data("MSSQL_PASSWORD is not valid UTF-8".to_owned())
            }
        })?;
        let mut configuration = MsSqlConfig::new();
        configuration.host(&options.host);
        configuration.port(options.port);
        configuration.database(&options.database);
        configuration.authentication(AuthMethod::sql_server(&options.user, password));
        configuration.application_name("open-sdbl");
        configuration.readonly(true);
        if connection.trust_server_certificate {
            configuration.trust_cert();
        }

        let stream = if let Some(proxy) = &options.socks5_proxy {
            connect_socks5(proxy, &options.host, options.port).await?
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
            client,
            database: options.database.clone(),
            year_offset: 0,
        };
        session
            .execute_batch(
                "SET QUOTED_IDENTIFIER ON; SET TRANSACTION ISOLATION LEVEL READ COMMITTED;",
            )
            .await?;
        session.verify_database().await?;
        session.year_offset = session.read_year_offset().await?;
        Ok(session)
    }

    async fn execute_batch(&mut self, sql: &str) -> Result<(), CliError> {
        self.client
            .simple_query(sql)
            .await
            .map_err(CliError::mssql_query)?
            .into_results()
            .await
            .map_err(CliError::mssql_query)?;
        Ok(())
    }

    async fn verify_database(&mut self) -> Result<(), CliError> {
        let rows = self
            .client
            .simple_query(MsSqlMetadataQueries::VERIFY_DATABASE)
            .await
            .map_err(CliError::mssql_query)?
            .into_first_result()
            .await
            .map_err(CliError::mssql_query)?;
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
        let rows = mssql_rows(&mut self.client, MsSqlMetadataQueries::YEAR_OFFSET).await?;
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

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        self.execute_batch("BEGIN TRANSACTION").await?;
        let result = acquire_mssql_metadata(&mut self.client).await;
        match result {
            Ok(snapshot) => {
                self.execute_batch("COMMIT TRANSACTION").await?;
                Ok(snapshot)
            }
            Err(error) => {
                let _ = self.execute_batch("ROLLBACK TRANSACTION").await;
                Err(error)
            }
        }
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, CliError> {
        self.execute_batch("BEGIN TRANSACTION").await?;
        let result = async {
            let rows = self
                .client
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
        }
        .await;
        match result {
            Ok(rows) => {
                self.execute_batch("COMMIT TRANSACTION").await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = self.execute_batch("ROLLBACK TRANSACTION").await;
                Err(error)
            }
        }
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

async fn connect_socks5(
    proxy: &Socks5Proxy,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, CliError> {
    let request =
        socks5_connect_request(target_host, target_port).map_err(CliError::socks5_connection)?;
    let negotiation = async {
        let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port)).await?;

        stream.write_all(&[0x05, 0x01, 0x00]).await?;
        let mut method = [0_u8; 2];
        stream.read_exact(&mut method).await?;
        match method {
            [0x05, 0x00] => {}
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
        if response[2] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proxy returned a malformed SOCKS5 response",
            ));
        }
        if response[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                socks5_reply_message(response[1]),
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
    let db_names_rows = transaction
        .query(PostgresMetadataQueries::DB_NAMES, &[])
        .await?;
    let db_names_data: Vec<u8> = exactly_one_row(&db_names_rows, "DBNames")?.try_get(0)?;
    let db_names = run_metadata_blocking("DBNames", move || {
        parse_db_names(&db_names_data).map_err(CliError::from)
    })
    .await?;

    let totals = transaction
        .query_one(PostgresMetadataQueries::CONFIG_TOTALS, &[])
        .await?;
    let total_resources = unsigned_progress_total(totals.try_get(0)?, "resource count")?;
    let total_bytes = unsigned_progress_total(totals.try_get(1)?, "compressed byte count")?;
    progress.config_totals(total_resources, total_bytes);

    let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
    let rows = transaction
        .query_raw(PostgresMetadataQueries::CONFIG, parameters)
        .await?;
    let resources = rows.map(|row| {
        let row = row?;
        Ok(ConfigResource {
            file_name: row.try_get(0)?,
            compressed: row.try_get(1)?,
        })
    });
    let (descriptors, predefined_values) = decode_config_stream(
        resources,
        CONFIG_DECODE_BATCH_SIZE,
        config_pipeline_depth(),
        &mut progress,
    )
    .await?;

    progress.phase("SchemaStorage");
    let schema_rows = transaction
        .query(PostgresMetadataQueries::SCHEMA, &[])
        .await?;
    let schema_data: Vec<u8> = exactly_one_row(&schema_rows, "SchemaStorage")?.try_get(0)?;
    let schema = run_metadata_blocking("SchemaStorage", move || {
        parse_schema_storage(&schema_data).map_err(CliError::from)
    })
    .await?;

    progress.phase("catalog");
    let catalog_rows = transaction
        .query(PostgresMetadataQueries::CATALOG, &[])
        .await?;
    let live_tables = run_metadata_blocking("PostgreSQL catalog", move || {
        decode_catalog_rows(catalog_rows)
    })
    .await?;

    progress.phase("resolve");
    let snapshot = run_metadata_blocking("metadata resolution", move || {
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
    Ok(snapshot)
}

async fn acquire_mssql_metadata(
    client: &mut MsSqlClient<MsSqlTransport>,
) -> Result<MetadataSnapshot, CliError> {
    let mut progress = MetadataProgress::new();
    progress.phase("DBNames");
    let db_names_rows = mssql_rows(client, MsSqlMetadataQueries::DB_NAMES).await?;
    let db_names_data = required_mssql_bytes(
        exactly_one_mssql_row(&db_names_rows, "DBNames")?,
        0,
        "DBNames payload",
    )?;
    let db_names = run_metadata_blocking("DBNames", move || {
        parse_db_names(&db_names_data).map_err(CliError::from)
    })
    .await?;

    let totals = mssql_rows(client, MsSqlMetadataQueries::CONFIG_TOTALS).await?;
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

    let config_rows = client
        .simple_query(MsSqlMetadataQueries::CONFIG)
        .await
        .map_err(CliError::mssql_query)?
        .into_row_stream();
    let resources = config_rows.map(|row| {
        let row = row.map_err(CliError::mssql_query)?;
        Ok(ConfigResource {
            file_name: required_mssql_string(&row, 0, "Config file name")?,
            compressed: required_mssql_bytes(&row, 1, "Config payload")?,
        })
    });
    let (descriptors, predefined_values) = decode_config_stream(
        resources,
        CONFIG_DECODE_BATCH_SIZE,
        config_pipeline_depth(),
        &mut progress,
    )
    .await?;

    progress.phase("SchemaStorage");
    let schema_rows = mssql_rows(client, MsSqlMetadataQueries::SCHEMA).await?;
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
    let catalog_rows = mssql_rows(client, MsSqlMetadataQueries::CATALOG).await?;
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
    let snapshot = run_metadata_blocking("metadata resolution", move || {
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
    Ok(snapshot)
}

async fn mssql_rows(
    client: &mut MsSqlClient<MsSqlTransport>,
    sql: &str,
) -> Result<Vec<tiberius::Row>, CliError> {
    client
        .simple_query(sql)
        .await
        .map_err(CliError::mssql_query)?
        .into_first_result()
        .await
        .map_err(CliError::mssql_query)
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
    let transaction_mode = transaction
        .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
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

fn postgres_password(connection: &PostgresConnection) -> Result<Option<String>, CliError> {
    if let Some(password) = env::var_os("PGPASSWORD") {
        return password
            .into_string()
            .map(Some)
            .map_err(|_| CliError::Data("PGPASSWORD is not valid UTF-8".to_owned()));
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
) -> Result<Option<String>, CliError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliError::Io(
                format!("cannot inspect PostgreSQL password file {path:?}"),
                error,
            ));
        }
    };
    reject_insecure_password_file(path, &metadata)?;
    let contents = fs::read_to_string(path).map_err(|error| {
        CliError::Io(
            format!("cannot read PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    Ok(contents.lines().find_map(|line| {
        let fields = parse_password_line(line)?;
        matches_password_field(&fields[0], &connection.host)
            .then_some(())
            .filter(|_| matches_password_field(&fields[1], &connection.port.to_string()))
            .filter(|_| matches_password_field(&fields[2], &connection.database))
            .filter(|_| matches_password_field(&fields[3], &connection.user))
            .map(|()| fields[4].clone())
    }))
}

#[cfg(unix)]
fn reject_insecure_password_file(path: &Path, metadata: &fs::Metadata) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must have permissions 0600 or stricter"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_password_file(_path: &Path, _metadata: &fs::Metadata) -> Result<(), CliError> {
    Ok(())
}

fn parse_password_line(line: &str) -> Option<[String; 5]> {
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
    fields.try_into().ok()
}

fn matches_password_field(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

fn print_snapshot(snapshot: MetadataSnapshot) {
    println!("RECORD\tGUID\tKIND\tNAME\tPHYSICAL_NAME\tOWNER\tSCHEMA\tLIVE\tDETAIL");
    for object in snapshot.objects {
        let mut details = Vec::new();
        if let Some(allowed_length) = object.code_allowed_length {
            details.push(format!("Code={}", allowed_length.as_str()));
        }
        if let Some(allowed_length) = object.number_allowed_length {
            details.push(format!("Number={}", allowed_length.as_str()));
        }
        println!(
            "OBJECT\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}",
            object.guid,
            object.kind.map_or("NonTabular", |kind| kind.as_str()),
            escape_field(object.name.as_deref().unwrap_or("")),
            object.physical_table.as_deref().unwrap_or(""),
            yes_no(object.declared),
            yes_no(object.live),
            details.join(","),
        );
    }
    for field in snapshot.fields {
        println!(
            "FIELD\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            field.guid,
            if field.data_separator {
                "DataSeparator"
            } else {
                "Field"
            },
            escape_field(field.name.as_deref().unwrap_or("")),
            field.physical_name,
            field.owner_tables.join(","),
            yes_no(field.declared),
            yes_no(field.live),
        );
    }
    for index in snapshot.indexes {
        println!(
            "INDEX\t\tIndex\t{}\t{}\t{}\tyes\t{}\t{}",
            escape_field(&index.declared_name),
            index.live_name.as_deref().unwrap_or(""),
            index.table,
            yes_no(index.live_name.is_some() && index.unique_matches),
            index.logical_key.join(","),
        );
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn lex(path: &str) -> Result<(), CliError> {
    let source = if path == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?;
        source
    } else {
        fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path:?}"), error))?
    };

    for token in tokenize(&source).map_err(CliError::Lexical)? {
        println!(
            "{}:{}\t{}\t{}",
            token.span.line,
            token.span.column,
            token.kind,
            escape_lexeme(token.lexeme)
        );
    }
    Ok(())
}

fn escape_lexeme(lexeme: &str) -> String {
    let mut escaped = String::with_capacity(lexeme.len());
    for character in lexeme.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
    Metadata(MetadataError),
    Data(String),
    Database(String),
    Terminal(String),
}

impl CliError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) | Self::Metadata(_) | Self::Data(_) => 1,
            Self::Usage(_) | Self::Io(_, _) | Self::Database(_) | Self::Terminal(_) => 2,
        }
    }

    fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL connection failed: {error}"))
    }

    fn mssql_connection(error: tiberius::error::Error) -> Self {
        Self::Database(format!("MSSQL connection failed: {error}"))
    }

    fn mssql_query(error: tiberius::error::Error) -> Self {
        Self::Database(format!("MSSQL query failed: {error}"))
    }

    fn socks5_connection(error: impl fmt::Display) -> Self {
        Self::Database(format!("SOCKS5 proxy connection failed: {error}"))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
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
    use super::{
        ConfigResource, ConnectionOptions, DatabaseConnection, MetadataProgress, MsSqlConnection,
        MsSqlSession, PostgresConnection, Socks5Proxy, connect_postgres_raw, connect_socks5,
        decode_catalog_values, decode_config_stream, format_mssql_binary, parse_connection,
        parse_password_line, parse_socks5_proxy, read_password_file, render_metadata_progress,
        socks5_connect_request,
    };
    use open_sdbl::metadata::{FieldId, MetadataSnapshot, StandardFieldId};
    use open_sdbl::query::{
        CompiledQuery, PresentationExpression, PresentationPlan,
        compile_mssql_query_with_year_offset, prepare_mssql_query_with_year_offset,
    };

    fn compile_mssql_test_query(
        source: &str,
        snapshot: &MetadataSnapshot,
        year_offset: i32,
    ) -> CompiledQuery {
        let prepared = prepare_mssql_query_with_year_offset(source, snapshot, year_offset).unwrap();
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
        }
    }

    #[tokio::test]
    #[ignore = "requires OPEN_SDBL_MSSQL_TEST_USER, MSSQL_PASSWORD, and a live 1C database"]
    async fn reads_metadata_from_the_mssql_demo_database() {
        let mut session = MsSqlSession::connect(&mssql_test_connection())
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
        let mut session = MsSqlSession::connect(&mssql_test_connection())
            .await
            .unwrap();
        let year_offset = session.year_offset;
        let snapshot = session.metadata().await.unwrap();
        let compiled = compile_mssql_query_with_year_offset(
            "SELECT Version FROM Документ._ДемоЗаказПокупателя WHERE Version > 0x00000000000007D6;",
            &snapshot,
            year_offset,
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
        let mut session = MsSqlSession::connect(&mssql_test_connection())
            .await
            .unwrap();
        let snapshot = session.metadata().await.unwrap();
        let direct = compile_mssql_test_query(
            "SELECT TOP 3 ID, Code, Description FROM Catalog._ДемоНоменклатура;",
            &snapshot,
            session.year_offset,
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
            session.year_offset,
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
            session.year_offset,
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
        let connection = parse_connection(&mut arguments, "metadata")
            .unwrap()
            .unwrap();
        let DatabaseConnection::MsSql(connection) = connection else {
            panic!("expected MSSQL connection");
        };
        assert_eq!(connection.options.host, "192.168.122.222");
        assert_eq!(connection.options.port, 1433);
        assert_eq!(connection.options.database, "demo");
        assert!(connection.trust_server_certificate);
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
        let error = parse_connection(&mut arguments, "console").unwrap_err();
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

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let high = char::from(pair[0]).to_digit(16).unwrap();
                let low = char::from(pair[1]).to_digit(16).unwrap();
                ((high << 4) | low) as u8
            })
            .collect()
    }

    #[test]
    fn parses_socks5_proxy_endpoints() {
        assert_eq!(
            parse_socks5_proxy("proxy.example:1080").unwrap(),
            Socks5Proxy {
                host: "proxy.example".to_owned(),
                port: 1080,
            }
        );
        assert_eq!(
            parse_socks5_proxy("[2001:db8::1]:9050").unwrap(),
            Socks5Proxy {
                host: "2001:db8::1".to_owned(),
                port: 9050,
            }
        );
        assert!(parse_socks5_proxy("proxy.example").is_err());
        assert!(parse_socks5_proxy("2001:db8::1:1080").is_err());
        assert!(parse_socks5_proxy(":1080").is_err());
        assert!(parse_socks5_proxy("proxy.example:0").is_err());
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
        };
        let stream = connect_socks5(&proxy, "database.internal", 15432)
            .await
            .unwrap();
        drop(stream);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn reports_unsupported_socks5_authentication() {
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
        };

        let error = connect_socks5(&proxy, "database.internal", 5432)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported authentication method 0x02"));
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
        let fields = parse_password_line(r"host\:part:5432:*:reader:pa\\ss\:word").unwrap();
        assert_eq!(fields[0], "host:part");
        assert_eq!(fields[2], "*");
        assert_eq!(fields[4], r"pa\ss:word");
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
        let connection = PostgresConnection(ConnectionOptions {
            host: "db".to_owned(),
            port: 5432,
            database: "test".to_owned(),
            user: "reader".to_owned(),
            socks5_proxy: None,
        });

        assert_eq!(
            read_password_file(&path, &connection, true).unwrap(),
            Some("secret".to_owned())
        );
        std::fs::remove_file(path).unwrap();
    }
}
