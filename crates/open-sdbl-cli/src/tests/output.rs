//! Tests of the `output` module.

use super::{MAX_CELL_WIDTH, bounded_field, escape_field, write_top_level_error};
use crate::error::CliError;

#[test]
fn escapes_controls_and_truncates_at_character_boundaries() {
    assert_eq!(escape_field("\\\t\r\n"), "\\\\\\t\\r\\n");
    assert_eq!(escape_field("\x1b[2J"), "\\u{1b}[2J");
    assert_eq!(
        escape_field("\x1b]52;c;payload\x07"),
        "\\u{1b}]52;c;payload\\u{7}"
    );
    // Line and paragraph separators and the bidirectional overrides
    // would reorder a terminal line around the escaped text.
    assert_eq!(
        escape_field("a\u{2028}\u{2029}\u{202e}b\u{2066}c\u{2069}"),
        "a\\u{2028}\\u{2029}\\u{202e}b\\u{2066}c\\u{2069}"
    );
    assert!(bounded_field("界界", 3).ends_with('…'));
    let wide = "界".repeat(MAX_CELL_WIDTH);
    let bounded = bounded_field(&wide, MAX_CELL_WIDTH);
    assert!(unicode_width::UnicodeWidthStr::width(bounded.as_str()) <= MAX_CELL_WIDTH);
    assert!(bounded.ends_with('…'));
}

#[test]
fn preserves_only_trusted_usage_layout() {
    let mut usage = Vec::new();
    write_top_level_error(&mut usage, &CliError::Usage("error\n\nhelp".to_owned())).unwrap();
    assert_eq!(usage, b"error\n\nhelp\n");

    let mut structured = Vec::new();
    write_top_level_error(&mut structured, &CliError::PostgresPlaintextOptInRequired).unwrap();
    assert!(structured.starts_with(b"error[OPEN_SDBL_CLI_PG_PLAINTEXT_OPT_IN_REQUIRED]:"));
    assert!(structured.ends_with(b"transport security\n"));

    let mut external = Vec::new();
    write_top_level_error(&mut external, &CliError::Data("bad\n\x1b[2J".to_owned())).unwrap();
    assert_eq!(external, b"bad\\n\\u{1b}[2J\n");
}
