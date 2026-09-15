//! Tests of the `error` module.

use std::io;

use super::CliError;

#[test]
fn recognizes_broken_standard_output() {
    let error = CliError::standard_output(io::Error::from(io::ErrorKind::BrokenPipe));
    assert!(error.is_broken_pipe());
}
