use std::time::Duration;

use open_sdbl_db::{
    BlockingDatabaseSession, Cell, ConnectionOptions, Credentials, DatabaseConnection,
    DatabaseSession, DbError, Limits, NoProgress, PostgresConnection, PostgresSslMode, RowFlow,
};

// Контроль работает только на отдельном собственном кластере, без доступа к исходным базам.
fn target() -> Option<DatabaseConnection> {
    let port = std::env::var("OPEN_SDBL_TRANSACTION_CONTROL_PORT").ok()?;
    Some(DatabaseConnection::Postgres(PostgresConnection {
        options: ConnectionOptions {
            host: "127.0.0.1".into(),
            port: port.parse().unwrap(),
            database: "postgres".into(),
            user: "sdbl_control".into(),
            socks5_proxy: None,
        },
        sslmode: PostgresSslMode::Disable,
        trust_ca_file: None,
    }))
}
fn credentials() -> Credentials {
    Credentials::new(None, None, None)
}
const IDENTITY: &str = "SELECT pg_backend_pid()::text, txid_current()::text";

#[tokio::test]
async fn async_lifecycle_reuses_connection_and_transaction_and_isolates_sessions() {
    let Some(target) = target() else {
        eprintln!("transaction control: own PostgreSQL is not configured");
        return;
    };
    let mut a = DatabaseSession::connect(&target, &credentials(), Limits::default())
        .await
        .unwrap();
    let mut b = DatabaseSession::connect(&target, &credentials(), Limits::default())
        .await
        .unwrap();
    assert!(!a.transaction_active().await.unwrap());
    assert!(a.commit_transaction().await.is_err());
    assert!(a.rollback_transaction().await.is_err());
    a.begin_read_only_transaction().await.unwrap();
    assert!(a.transaction_active().await.unwrap());
    assert!(a.begin_read_only_transaction().await.is_err());
    let first = a.query(IDENTITY, 2).await.unwrap();
    assert_eq!(first, a.query(IDENTITY, 2).await.unwrap());
    assert!(a.metadata(&mut NoProgress).await.is_err());
    assert!(a.configuration(&mut NoProgress).await.is_err());
    assert_eq!(first, a.query(IDENTITY, 2).await.unwrap());
    a.query_each("SELECT generate_series(1, 100)", 1, |_| Ok(RowFlow::Stop))
        .await
        .unwrap();
    assert!(a.transaction_active().await.unwrap());
    b.begin_read_only_transaction().await.unwrap();
    let second = b.query(IDENTITY, 2).await.unwrap();
    assert_ne!(first[0][0], second[0][0]);
    assert_ne!(first[0][1], second[0][1]);
    a.commit_transaction().await.unwrap();
    assert!(!a.transaction_active().await.unwrap());
    assert!(b.transaction_active().await.unwrap());
    b.rollback_transaction().await.unwrap();
    assert!(!b.transaction_active().await.unwrap());
    assert_eq!(
        a.query("SELECT current_setting('transaction_read_only')", 1)
            .await
            .unwrap(),
        vec![vec![Cell::Text("on".into())]]
    );
    a.begin_read_only_transaction().await.unwrap();
    a.cancel_query(a.cancellation()).await.unwrap();
    assert!(!a.transaction_active().await.unwrap());
    assert_eq!(a.query("SELECT 1", 1).await.unwrap().len(), 1);
    a.close().await.unwrap();
    b.close().await.unwrap();
}

#[tokio::test]
async fn async_errors_and_timeouts_rollback_actual_transaction() {
    let Some(target) = target() else {
        return;
    };
    let limits = Limits {
        query_timeout: Duration::from_millis(200),
        ..Limits::default()
    };
    let mut a = DatabaseSession::connect(&target, &credentials(), limits)
        .await
        .unwrap();
    for sql in [
        "SELECT 1/0",
        "CREATE TABLE sdbl_forbidden_write (value integer)",
        "SELECT pg_sleep(2)",
    ] {
        a.begin_read_only_transaction().await.unwrap();
        assert!(a.query(sql, 1).await.is_err());
        assert!(!a.transaction_active().await.unwrap());
        assert!(!a.is_dead());
        assert_eq!(a.query("SELECT 1", 1).await.unwrap().len(), 1);
    }
    a.begin_read_only_transaction().await.unwrap();
    assert!(
        a.query_each("SELECT 1", 1, |_| Err(DbError::Data(
            "callback failure".into()
        )))
        .await
        .is_err()
    );
    assert!(!a.transaction_active().await.unwrap());
    a.close().await.unwrap();
}

#[tokio::test]
async fn abandoned_async_operation_refuses_reuse_and_never_reports_active() {
    let Some(target) = target() else {
        return;
    };
    let mut a = DatabaseSession::connect(&target, &credentials(), Limits::default())
        .await
        .unwrap();
    a.begin_read_only_transaction().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), a.query("SELECT pg_sleep(2)", 1))
            .await
            .is_err()
    );
    assert!(a.is_dead());
    assert!(a.transaction_active().await.is_err());
    assert!(a.query("SELECT 1", 1).await.is_err());
    // Отмена драйвера остаётся диагностикой close, но соединение уже невосстановимо.
    assert!(a.close().await.is_err());
}

#[test]
fn blocking_lifecycle_and_runtime_rules_match_async_session() {
    let Some(target) = target() else {
        return;
    };
    let mut a =
        BlockingDatabaseSession::connect(&target, &credentials(), Limits::default()).unwrap();
    assert!(!a.transaction_active().unwrap());
    assert!(a.commit_transaction().is_err());
    a.begin_read_only_transaction().unwrap();
    let first = a.query(IDENTITY, 2).unwrap();
    assert_eq!(first, a.query(IDENTITY, 2).unwrap());
    assert!(a.transaction_active().unwrap());
    a.commit_transaction().unwrap();
    assert!(!a.transaction_active().unwrap());
    a.begin_read_only_transaction().unwrap();
    a.rollback_transaction().unwrap();
    a.begin_read_only_transaction().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(a.transaction_active(), Err(DbError::AsyncContext)));
    });
    assert!(a.transaction_active().unwrap());
    a.close().unwrap();
    for result in [
        a.begin_read_only_transaction(),
        a.commit_transaction(),
        a.rollback_transaction(),
    ] {
        assert!(matches!(result, Err(DbError::SessionClosed)));
    }
    assert!(matches!(
        a.transaction_active(),
        Err(DbError::SessionClosed)
    ));
}

async fn observer(port: u16) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::Config::new()
        .host("127.0.0.1")
        .port(port)
        .user("sdbl_control")
        .dbname("postgres")
        .connect(tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(connection);
    client
}

#[tokio::test]
async fn close_drop_and_connection_loss_cleanup_server_state() {
    let Some(target) = target() else {
        return;
    };
    let DatabaseConnection::Postgres(connection) = &target else {
        unreachable!()
    };
    let observer = observer(connection.port).await;
    for close in [true, false] {
        let mut session = DatabaseSession::connect(&target, &credentials(), Limits::default())
            .await
            .unwrap();
        session.begin_read_only_transaction().await.unwrap();
        let rows = session
            .query("SELECT pg_backend_pid()::text", 1)
            .await
            .unwrap();
        let pid: i32 = rows[0][0].as_text().unwrap().parse().unwrap();
        if close {
            session.close().await.unwrap();
        } else {
            drop(session);
        }
        let mut gone = false;
        for _ in 0..100 {
            let row = observer
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity WHERE pid = $1",
                    &[&pid],
                )
                .await
                .unwrap();
            if row.get::<_, i64>(0) == 0 {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            gone,
            "закрытый сеанс не должен оставлять транзакцию на сервере"
        );
    }
    let mut session = DatabaseSession::connect(&target, &credentials(), Limits::default())
        .await
        .unwrap();
    session.begin_read_only_transaction().await.unwrap();
    let rows = session
        .query("SELECT pg_backend_pid()::text", 1)
        .await
        .unwrap();
    let pid: i32 = rows[0][0].as_text().unwrap().parse().unwrap();
    observer
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .await
        .unwrap();
    assert!(session.transaction_active().await.is_err());
    assert!(session.is_dead());
    let _ = session.close().await;
}

#[tokio::test]
async fn explicit_query_cancellation_rolls_back_without_false_active_state() {
    let Some(target) = target() else {
        return;
    };
    let mut session = DatabaseSession::connect(&target, &credentials(), Limits::default())
        .await
        .unwrap();
    session.begin_read_only_transaction().await.unwrap();
    let open_sdbl_db::QueryCancellation::Postgres(token) = session.cancellation() else {
        unreachable!()
    };
    let cancel = async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        token.cancel_query(tokio_postgres::NoTls).await.unwrap();
    };
    let (result, ()) = tokio::join!(session.query("SELECT pg_sleep(2)", 1), cancel);
    assert!(result.is_err());
    assert!(!session.transaction_active().await.unwrap());
    assert_eq!(session.query("SELECT 1", 1).await.unwrap().len(), 1);
    session.close().await.unwrap();
}
