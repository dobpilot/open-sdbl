//! Tests of the blocking session owner.
//!
//! The driver is exercised over an artificial provider: a session whose
//! configuration and metadata reads run the real acquisition over the
//! in-memory source of the pipeline tests, and whose queries answer rows
//! naming the connection they ran on. The same provider is also driven
//! asynchronously, so the two paths can be compared result for result.

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use open_sdbl::metadata::{MetadataSnapshot, ResolutionReport};

use super::{BlockingDatabaseSession, BlockingDriver, SessionOperations};
use crate::cells::{Cell, QueryRows, RowFlow};
use crate::connection::{
    ConnectionOptions, DatabaseConnection, MsSqlConnection, PostgresConnection, PostgresSslMode,
};
use crate::credentials::Credentials;
use crate::error::DbError;
use crate::limits::Limits;
use crate::pipeline::configuration_tests::FakeSource;
use crate::pipeline::{AcquiredConfiguration, acquire_configuration, acquire_metadata};
use crate::progress::{MetadataProgress, NoProgress};

const PASSWORD: &str = "s3cret-пароль-42";

/// What an artificial session is opened against: a counter handing out
/// connection numbers, and a log of the connections closed.
#[derive(Default)]
struct FakeServer {
    connections: AtomicUsize,
    closed: std::sync::Mutex<Vec<usize>>,
}

struct FakeSession {
    server: Arc<FakeServer>,
    connection: usize,
}

impl SessionOperations for FakeSession {
    type Target = Arc<FakeServer>;

    async fn connect(
        target: &Arc<FakeServer>,
        _credentials: &Credentials,
        _limits: Limits,
    ) -> Result<Self, DbError> {
        Ok(Self {
            server: Arc::clone(target),
            connection: target.connections.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }

    async fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        acquire_configuration(&mut FakeSource::new().with_extensions(), progress).await
    }

    async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        let (snapshot, _layout, report) =
            acquire_metadata(&mut FakeSource::new(), progress).await?;
        Ok((snapshot, report))
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        if sql == "fail" {
            return Err(DbError::Database(format!(
                "connection {} refused the statement",
                self.connection
            )));
        }
        // A suspension point, so the answer really is driven by a runtime.
        tokio::task::yield_now().await;
        Ok(vec![vec![
            Cell::Text(format!(
                "connection {}",
                self.connection
            ));
            column_count
        ]])
    }

    async fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        mut on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        for row in 0..3 {
            let mut cells = self.query(sql, column_count).await?.remove(0);
            cells.push(Cell::Text(format!("row {row}")));
            if on_row(cells)? == RowFlow::Stop {
                break;
            }
        }
        Ok(())
    }

    async fn close(self) -> Result<(), DbError> {
        self.server.closed.lock().unwrap().push(self.connection);
        Ok(())
    }
}

fn credentials() -> Credentials {
    Credentials::new(None, None, None)
}

fn blocking(server: &Arc<FakeServer>) -> BlockingDriver<FakeSession> {
    BlockingDriver::connect(server, &credentials(), Limits::default()).unwrap()
}

/// The same operations, driven the asynchronous way on a runtime of the
/// test's own.
fn asynchronous<T>(
    server: &Arc<FakeServer>,
    operation: impl AsyncFnOnce(&mut FakeSession) -> T,
) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut session = FakeSession::connect(server, &credentials(), Limits::default())
                .await
                .unwrap();
            operation(&mut session).await
        })
}

fn same_configuration(left: &AcquiredConfiguration, right: &AcquiredConfiguration) {
    assert_eq!(left.metadata.fingerprint(), right.metadata.fingerprint());
    assert_eq!(left.report, right.report);
    assert_eq!(left.layout, right.layout);
    assert_eq!(left.config_resources, right.config_resources);
    assert_eq!(left.extensions, right.extensions);
}

#[test]
fn answers_what_the_asynchronous_path_answers() {
    // Each path gets a server of its own, so each runs on its connection 1
    // and the answers can be compared as they are.
    let fresh = || Arc::new(FakeServer::default());
    let mut driver = blocking(&fresh());

    let expected = asynchronous(&fresh(), async |session| {
        session.configuration(&mut NoProgress).await.unwrap()
    });
    let actual = driver.configuration(&mut NoProgress).unwrap();
    same_configuration(&actual, &expected);
    assert_eq!(actual.extensions.len(), 2);

    let (expected_snapshot, expected_report) = asynchronous(&fresh(), async |session| {
        session.metadata(&mut NoProgress).await.unwrap()
    });
    let (snapshot, report) = driver.metadata(&mut NoProgress).unwrap();
    assert_eq!(snapshot.fingerprint(), expected_snapshot.fingerprint());
    assert_eq!(report, expected_report);

    let expected = asynchronous(&fresh(), async |session| {
        session.query("SELECT 1", 2).await.unwrap()
    });
    assert_eq!(driver.query("SELECT 1", 2).unwrap(), expected);
    assert_eq!(
        expected,
        vec![vec![Cell::Text("connection 1".to_owned()); 2]]
    );

    let stop_at_two = |seen: &mut Vec<Vec<Cell>>, row| {
        seen.push(row);
        Ok(if seen.len() == 2 {
            RowFlow::Stop
        } else {
            RowFlow::Continue
        })
    };
    let expected = asynchronous(&fresh(), async |session| {
        let mut seen = Vec::new();
        session
            .query_each("SELECT 1", 1, |row| stop_at_two(&mut seen, row))
            .await
            .unwrap();
        seen
    });
    let mut seen = Vec::new();
    driver
        .query_each("SELECT 1", 1, |row| stop_at_two(&mut seen, row))
        .unwrap();
    assert_eq!(seen, expected);
    assert_eq!(seen.len(), 2, "stopping ends the read at once");
}

#[test]
fn answers_the_error_the_asynchronous_path_answers() {
    let mut driver = blocking(&Arc::new(FakeServer::default()));
    let blocking_error = driver.query("fail", 1).unwrap_err();
    let asynchronous_error = asynchronous(&Arc::new(FakeServer::default()), async |session| {
        session.query("fail", 1).await
    })
    .unwrap_err();
    assert!(matches!(blocking_error, DbError::Database(_)));
    assert!(matches!(asynchronous_error, DbError::Database(_)));
    assert_eq!(blocking_error.to_string(), asynchronous_error.to_string());
    assert_eq!(
        blocking_error.to_string(),
        "connection 1 refused the statement"
    );

    // An error `on_row` answers comes back unchanged.
    let error = driver
        .query_each("SELECT 1", 1, |_| {
            Err(DbError::Data("stop here".to_owned()))
        })
        .unwrap_err();
    assert!(matches!(&error, DbError::Data(message) if message == "stop here"));

    // A failed call does not close the session.
    assert_eq!(driver.query("SELECT 1", 1).unwrap().len(), 1);
}

#[test]
fn two_sessions_are_two_connections() {
    let server = Arc::new(FakeServer::default());
    let mut first = blocking(&server);
    let mut second = blocking(&server);
    assert_eq!(
        first.query("SELECT 1", 1).unwrap()[0][0],
        Cell::Text("connection 1".to_owned())
    );
    assert_eq!(
        second.query("SELECT 1", 1).unwrap()[0][0],
        Cell::Text("connection 2".to_owned())
    );

    first.close().unwrap();
    assert_eq!(*server.closed.lock().unwrap(), [1]);
    assert_eq!(
        second.query("SELECT 1", 1).unwrap()[0][0],
        Cell::Text("connection 2".to_owned()),
        "closing one session leaves the other usable"
    );
    second.close().unwrap();
    assert_eq!(*server.closed.lock().unwrap(), [1, 2]);
}

#[test]
fn every_call_after_close_answers_session_closed() {
    let server = Arc::new(FakeServer::default());
    let mut driver = blocking(&server);
    driver.close().unwrap();

    assert!(driver.session().is_none());
    assert!(
        driver
            .configuration(&mut NoProgress)
            .unwrap_err()
            .is_session_closed()
    );
    assert!(
        driver
            .metadata(&mut NoProgress)
            .unwrap_err()
            .is_session_closed()
    );
    assert!(driver.query("SELECT 1", 1).unwrap_err().is_session_closed());
    assert!(
        driver
            .query_each("SELECT 1", 1, |_| Ok(RowFlow::Continue))
            .unwrap_err()
            .is_session_closed()
    );
    let error = driver.close().unwrap_err();
    assert!(error.is_session_closed());
    assert_eq!(error.to_string(), "the database session is closed");
    assert_eq!(*server.closed.lock().unwrap(), [1], "closed once");
}

#[test]
fn refuses_to_block_inside_an_asynchronous_runtime() {
    let server = Arc::new(FakeServer::default());
    let mut driver = blocking(&server);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            driver.query("SELECT 1", 1),
            Err(DbError::AsyncContext)
        ));
        assert!(matches!(driver.close(), Err(DbError::AsyncContext)));
        assert!(matches!(
            BlockingDriver::<FakeSession>::connect(&server, &credentials(), Limits::default()),
            Err(DbError::AsyncContext)
        ));
    });
    // Refusing to close left the session open.
    assert_eq!(driver.query("SELECT 1", 1).unwrap().len(), 1);

    // Dropped on a thread driving another runtime, the driver does not
    // panic.
    let dropped = blocking(&server);
    runtime.block_on(async move { drop(dropped) });
}

#[test]
fn a_closed_session_answers_session_closed_inside_a_runtime_too() {
    let server = Arc::new(FakeServer::default());
    let mut driver = blocking(&server);
    driver.close().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(
            driver
                .configuration(&mut NoProgress)
                .unwrap_err()
                .is_session_closed()
        );
        assert!(
            driver
                .metadata(&mut NoProgress)
                .unwrap_err()
                .is_session_closed()
        );
        assert!(driver.query("SELECT 1", 1).unwrap_err().is_session_closed());
        assert!(
            driver
                .query_each("SELECT 1", 1, |_| Ok(RowFlow::Continue))
                .unwrap_err()
                .is_session_closed()
        );
        assert!(driver.close().unwrap_err().is_session_closed());
    });
    assert_eq!(*server.closed.lock().unwrap(), [1], "closed once");
}

#[test]
fn a_session_can_move_to_another_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<BlockingDatabaseSession>();

    let server = Arc::new(FakeServer::default());
    let mut driver = blocking(&server);
    let rows = std::thread::spawn(move || driver.query("SELECT 1", 1))
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(rows.len(), 1);
}

/// A port nothing listens on: bound, then released.
fn refused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn options(port: u16) -> ConnectionOptions {
    ConnectionOptions {
        host: "127.0.0.1".to_owned(),
        port,
        database: "base".to_owned(),
        user: "reader".to_owned(),
        socks5_proxy: None,
    }
}

#[test]
fn a_refused_connection_does_not_carry_the_password() {
    let limits = Limits {
        connection_timeout: Duration::from_secs(5),
        ..Limits::default()
    };
    let credentials = Credentials::new(
        Some(PASSWORD.to_owned()),
        Some(PASSWORD.to_owned()),
        Some(PASSWORD.to_owned()),
    );
    for connection in [
        DatabaseConnection::Postgres(PostgresConnection {
            options: options(refused_port()),
            sslmode: PostgresSslMode::Disable,
            trust_ca_file: None,
        }),
        DatabaseConnection::MsSql(MsSqlConnection {
            options: options(refused_port()),
            trust_server_certificate: true,
            trust_ca_file: None,
            dialect_level: None,
        }),
    ] {
        let described = format!("{connection:?}");
        assert!(!described.contains(PASSWORD), "{described}");
        let error = BlockingDatabaseSession::connect(&connection, &credentials, limits)
            .expect_err("nothing listens on the port");
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(!text.contains(PASSWORD), "{text}");
        }
    }
}

#[test]
fn the_blocking_module_carries_no_second_sql_path() {
    let source = include_str!("../blocking.rs");
    for foreign in [
        "SELECT",
        "MetadataQueries",
        "PostgresSession",
        "MsSqlSession",
        "tokio_postgres",
        "tiberius",
        "acquire_",
        "decode_",
    ] {
        assert!(
            !source.contains(foreign),
            "the blocking module names {foreign}"
        );
    }
}
