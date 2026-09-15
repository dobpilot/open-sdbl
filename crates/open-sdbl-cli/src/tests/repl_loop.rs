//! Tests of the console `loop` module.

use std::time::Duration;


use super::*;

#[test]
fn exits_after_an_error_when_the_database_session_is_dead() {
    assert!(ensure_session_remains_usable(true).is_err());
    assert!(ensure_session_remains_usable(false).is_ok());
}

#[test]
fn formats_sql_generation_duration_compactly() {
    assert_eq!(format_duration(Duration::from_nanos(750)), "750 ns");
    assert_eq!(format_duration(Duration::from_micros(42)), "42 µs");
    assert_eq!(format_duration(Duration::from_micros(1_250)), "1.250 ms");
    assert_eq!(
        timing_line("PostgreSQL execution", Duration::from_micros(42)),
        "PostgreSQL execution: 42 µs"
    );
}
