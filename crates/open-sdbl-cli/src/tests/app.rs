//! Tests of the command controller.

use super::run_lex;
use crate::args::HELP;
use crate::output::{MAX_PRINTED_ROWS, lex};

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

#[test]
fn lex_help_does_not_read_standard_input() {
    let mut output = Vec::new();
    run_lex(std::iter::once("--help".to_owned()), &mut output).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), HELP);
}
