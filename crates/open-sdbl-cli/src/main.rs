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
mod cells;
mod db;
mod error;
mod net;
mod output;
mod params;
mod pipeline;
mod progress;
mod repl;
mod restrict;

use args::{DatabaseConnection, HELP, parse_connection};
use auth::pgpass::Credentials;
use db::mssql::MsSqlSession;
use db::postgres::PostgresSession;
use error::CliError;
use output::{
    MAX_CELL_WIDTH, MAX_PRINTED_ROWS, bounded_field, escape_field, lex, print_snapshot,
    read_lex_source, write_top_level_error, yes_no,
};

/// The hex decoder the library tests use, shared so that a fixture written
/// as hex reads the same way on both sides.
#[cfg(test)]
#[path = "../../../tests/support/hex.rs"]
mod hex_test_support;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(120);
const POSTGRES_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const CONFIG_DECODE_BATCH_SIZE: usize = 256;
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

pub(crate) type QueryRows = Vec<Vec<cells::Cell>>;

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

    /// Provider-specific startup line, when the provider has one.
    pub(crate) fn server_description(&self) -> Option<String> {
        match self {
            Self::Postgres(_) => None,
            Self::MsSql(session) => Some(session.server_description()),
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

    use super::{CliError, HELP, MAX_PRINTED_ROWS, bounded_database_call, lex, run_lex};

    /// The width of one cell is bounded by `output`, which owns that rule
    /// and tests it; here only the row budget of the `lex` command is.
    #[test]
    fn bounds_lex_rows() {
        let source = std::iter::repeat_n("x", MAX_PRINTED_ROWS + 1)
            .collect::<Vec<_>>()
            .join(" ");
        let tokens = open_sdbl::tokenize(&source).unwrap();
        let mut output = Vec::new();
        lex(&mut output, &tokens).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.ends_with("# 1 rows omitted\n"));
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

    #[test]
    fn lex_help_does_not_read_standard_input() {
        let mut output = Vec::new();
        run_lex(std::iter::once("--help".to_owned()), &mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), HELP);
    }
}
