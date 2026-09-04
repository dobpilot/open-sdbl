use std::env;
use std::future::Future;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;
use std::time::Duration;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::MsSqlBackend;
use open_sdbl::tokenize;
use tokio::time::timeout;

mod args;
mod auth;
mod db;
mod error;
mod net;
mod output;
mod pipeline;
mod progress;
mod repl;

#[cfg(test)]
use args::{
    ConnectionOptions, INSECURE_MSSQL_CERTIFICATE_WARNING, MsSqlConnection, PostgresConnection,
    PostgresSslMode, select_postgres_sslmode,
};
use args::{DatabaseConnection, HELP, parse_connection};
use auth::pgpass::Credentials;
#[cfg(all(test, unix))]
use auth::pgpass::reject_password_file_owner;
#[cfg(test)]
use auth::pgpass::{EnvironmentSecret, parse_password_line, read_password_file};
use db::mssql::MsSqlSession;
#[cfg(test)]
use db::mssql::{apply_mssql_cleanup, format_mssql_binary};
use db::postgres::PostgresSession;
#[cfg(test)]
use db::postgres::{await_postgres_driver, connect_postgres_raw};
use error::CliError;
#[cfg(test)]
use net::socks5::{Socks5Proxy, connect_socks5, parse_socks5_proxy, socks5_connect_request};
use output::{
    MAX_CELL_WIDTH, MAX_PRINTED_ROWS, bounded_field, escape_field, lex, print_snapshot,
    read_lex_source, write_top_level_error, yes_no,
};
#[cfg(test)]
use pipeline::{ConfigDecodeLimits, ConfigResource, decode_catalog_values, decode_config_stream};
#[cfg(test)]
use progress::{MetadataProgress, render_metadata_progress};

#[cfg(test)]
#[path = "../../../tests/support/hex.rs"]
mod hex_test_support;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(120);
const POSTGRES_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const CONFIG_DECODE_BATCH_SIZE: usize = 256;
const MSSQL_VERIFY_READONLY: &str = "SELECT CONVERT(int, @@TRANCOUNT), CONVERT(int, CASE WHEN ISNULL(IS_MEMBER(N'db_datareader'), 0) = 1 AND ISNULL(IS_MEMBER(N'db_datawriter'), 0) = 0 AND ISNULL(IS_MEMBER(N'db_owner'), 0) = 0 AND ISNULL(IS_SRVROLEMEMBER(N'sysadmin'), 0) = 0 THEN 1 ELSE 0 END), CONVERT(int, transaction_isolation_level) FROM sys.dm_exec_sessions WHERE session_id = @@SPID";
const MSSQL_TRANSACTION_COUNT: &str = "SELECT CONVERT(int, @@TRANCOUNT)";

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
            let stderr = io::stderr();
            let _ = write_top_level_error(&mut stderr.lock(), &error);
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
        "lex" => run_lex(arguments, output),
        "metadata" => metadata(arguments, output, credentials).await,
        "console" | "repl" => console(arguments, output, credentials).await,
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

fn run_lex(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let path = arguments.next().unwrap_or_else(|| "-".to_owned());
    if matches!(path.as_str(), "-h" | "--help") {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(());
    }
    if let Some(unexpected) = arguments.next() {
        return Err(CliError::Usage(format!(
            "unexpected argument {unexpected:?}\n\n{HELP}"
        )));
    }
    let source = read_lex_source(&path)?;
    let tokens = tokenize(&source).map_err(CliError::Lexical)?;
    lex(output, &tokens).map_err(CliError::standard_output)
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

async fn bounded_database_call<T>(
    label: &str,
    duration: Duration,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    timeout(duration, future)
        .await
        .map_err(|_| CliError::DatabaseTimeout {
            operation: label.to_owned(),
            duration,
        })?
}

async fn query_timeout<T>(
    label: &str,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    bounded_database_call(label, QUERY_TIMEOUT, future).await
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
                backend: session.backend(),
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
            Self::Postgres(session) => session.is_closed(),
            Self::MsSql(session) => session.is_dead(),
        }
    }

    pub(crate) fn cancellation(&self) -> QueryCancellation {
        match self {
            Self::Postgres(session) => QueryCancellation::Postgres(session.cancellation()),
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

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use zeroize::Zeroizing;

    use super::hex_test_support::hex;
    #[cfg(unix)]
    use super::reject_password_file_owner;
    use super::{
        CliError, ConfigDecodeLimits, ConfigResource, ConnectionOptions, Credentials,
        DatabaseConnection, EnvironmentSecret, HELP, INSECURE_MSSQL_CERTIFICATE_WARNING,
        MAX_CELL_WIDTH, MAX_PRINTED_ROWS, MSSQL_VERIFY_READONLY, MetadataProgress, MsSqlConnection,
        MsSqlSession, PostgresConnection, PostgresSslMode, Socks5Proxy, apply_mssql_cleanup,
        await_postgres_driver, bounded_database_call, bounded_field, connect_postgres_raw,
        connect_socks5, decode_catalog_values, decode_config_stream, escape_field,
        format_mssql_binary, lex, parse_connection, parse_password_line, parse_socks5_proxy,
        read_password_file, render_metadata_progress, run_lex, select_postgres_sslmode,
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
        assert!(!snapshot.objects().is_empty());
        assert!(!snapshot.live_tables().is_empty());
        session.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires the MSSQL demo database and its _ДемоЗаказПокупателя document"]
    async fn reads_native_rowversion_from_the_mssql_demo_database() {
        let mut session =
            MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
                .await
                .unwrap();
        let backend = session.backend();
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
            session.backend().year_offset(),
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
            session.backend().year_offset(),
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
            session.backend().year_offset(),
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
        assert!(error.is_database_timeout());
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
    fn accepts_inline_option_values_and_rejects_option_like_values() {
        let mut inline = ["postgres", "--host=db", "--database=test", "--user=reader"]
            .into_iter()
            .map(str::to_owned);
        let connection = parse_connection(&mut inline, "console", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::Postgres(connection) = connection else {
            panic!("expected PostgreSQL connection");
        };
        assert_eq!(connection.options.host, "db");

        let mut option_like = ["postgres", "--host", "--database=test", "--user=reader"]
            .into_iter()
            .map(str::to_owned);
        let error = parse_connection(&mut option_like, "console", &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("option-like"));
    }

    #[test]
    fn lex_help_does_not_read_standard_input() {
        let mut output = Vec::new();
        run_lex(std::iter::once("--help".to_owned()), &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), HELP);
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
    fn parses_private_ca_files_for_both_database_providers() {
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
        let connection = parse_connection(&mut arguments, "metadata", &mut Vec::new())
            .unwrap()
            .unwrap();
        let DatabaseConnection::Postgres(connection) = connection else {
            panic!("expected PostgreSQL connection");
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
            "--sslmode",
            "require",
            "--trust-ca-file",
            "company-ca.pem",
        ]
        .into_iter()
        .map(str::to_owned);
        let error = parse_connection(&mut arguments, "metadata", &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("requires PostgreSQL --sslmode"));
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
        let (descriptors, predefined_values) = decode_config_stream(
            resources,
            2,
            2,
            ConfigDecodeLimits::default(),
            &mut progress,
        )
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
        let error = decode_config_stream(
            invalid,
            2,
            2,
            ConfigDecodeLimits::default(),
            &mut MetadataProgress::disabled(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("DEFLATE"));

        let limited = futures_util::stream::iter([Ok(ConfigResource {
            file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
            compressed: compressed.clone(),
        })]);
        let error = decode_config_stream(
            limited,
            1,
            1,
            ConfigDecodeLimits {
                resource_bytes: 1,
                batch_bytes: 2,
                total_bytes: 2,
                in_flight_bytes: 2,
            },
            &mut MetadataProgress::disabled(),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("decoded metadata exceeds 1 byte limit")
        );

        let total_limited = futures_util::stream::iter([Ok(ConfigResource {
            file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
            compressed,
        })]);
        let error = decode_config_stream(
            total_limited,
            1,
            1,
            ConfigDecodeLimits {
                resource_bytes: 1024 * 1024,
                batch_bytes: 1024 * 1024,
                total_bytes: 1,
                in_flight_bytes: 1024 * 1024,
            },
            &mut MetadataProgress::disabled(),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Config total decoded size exceeds 1 byte")
        );
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
            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x02]);
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
    async fn rejects_socks5_authentication_downgrade() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut greeting = [0_u8; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [0x05, 0x01, 0x02]);
            stream.write_all(&[0x05, 0x00]).await.unwrap();
        });
        let proxy = Socks5Proxy {
            host: address.ip().to_string(),
            port: address.port(),
            username: Some("user".to_owned()),
        };

        let error = connect_socks5(&proxy, Some("secret"), "database.internal", 5432)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("despite configured credentials"));
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
            trust_ca_file: None,
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
        use std::os::unix::fs::{MetadataExt, symlink};

        let path = std::env::temp_dir().join(format!(
            "open-sdbl-pgpass-fifo-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path_bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: `path_bytes` is a valid NUL-terminated path and the mode is valid.
        assert_eq!(unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) }, 0);
        let connection = PostgresConnection {
            options: ConnectionOptions {
                host: "db".to_owned(),
                port: 5432,
                database: "test".to_owned(),
                user: "reader".to_owned(),
                socks5_proxy: None,
            },
            sslmode: PostgresSslMode::VerifyFull,
            trust_ca_file: None,
        };
        let metadata = std::fs::metadata(&path).unwrap();
        let error = read_password_file(&path, &connection, true).unwrap_err();
        assert!(error.to_string().contains("regular file"));
        std::fs::remove_file(&path).unwrap();

        let target = path.with_extension("target");
        let link = path.with_extension("link");
        std::fs::write(&target, "*:5432:test:reader:secret\n").unwrap();
        symlink(&target, &link).unwrap();
        let error = read_password_file(&link, &connection, true).unwrap_err();
        assert!(error.to_string().contains("cannot open"));
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(target).unwrap();

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
