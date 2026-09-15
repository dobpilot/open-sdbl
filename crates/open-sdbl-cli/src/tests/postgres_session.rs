//! Tests of the `postgres session` module.

use super::*;
use std::time::Duration;

#[tokio::test]
async fn aborts_a_stalled_driver_on_close() {
    let driver =
        tokio::spawn(async { std::future::pending::<Result<(), tokio_postgres::Error>>().await });
    assert!(
        await_postgres_driver(driver, Duration::from_millis(5))
            .await
            .unwrap_err()
            .to_string()
            .contains("driver aborted")
    );
}

#[tokio::test]
async fn aborts_a_stalled_postgres_driver_on_close() {
    let driver =
        tokio::spawn(async { std::future::pending::<Result<(), tokio_postgres::Error>>().await });
    let error = await_postgres_driver(driver, Duration::from_millis(20))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("driver aborted"));
}
