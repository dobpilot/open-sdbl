//! Tests of the `mssql session` module.

use super::apply_mssql_cleanup;
use super::*;
use crate::error::CliError;
use std::time::Duration;

#[test]
fn timeout_requires_dropping_mssql_without_rollback() {
    let timeout = CliError::DatabaseTimeout {
        operation: "MSSQL user query".to_owned(),
        duration: Duration::from_secs(120),
    };
    assert!(should_disconnect_after_mssql_error(&timeout));

    let semantic_error = CliError::Data("invalid row".to_owned());
    assert!(!should_disconnect_after_mssql_error(&semantic_error));
}

#[test]
fn failed_mssql_cleanup_poisons_the_session_state() {
    let mut poisoned = false;
    let error =
        apply_mssql_cleanup(&mut poisoned, Err("connection lost".to_owned()), Ok(0)).unwrap_err();
    assert!(poisoned);
    assert!(error.contains("ROLLBACK failed"));

    let mut poisoned = false;
    let error = apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(1)).unwrap_err();
    assert!(poisoned);
    assert!(error.contains("@@TRANCOUNT remained 1"));

    let mut poisoned = false;
    apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(0)).unwrap();
    assert!(!poisoned);
    // The session verifies transaction state, not role membership:
    // what rights the login holds is the operator's decision.
    assert!(crate::limits::MSSQL_TRANSACTION_COUNT.contains("@@TRANCOUNT"));
}
