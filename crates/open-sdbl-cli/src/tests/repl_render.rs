//! Tests of the console `render` module.


use unicode_width::UnicodeWidthStr;

use super::*;
use crate::output::{MAX_CELL_WIDTH, MAX_PRINTED_ROWS};

#[test]
fn aligns_cjk_by_display_columns() {
    assert_eq!(display_width("界"), 2);
    assert_eq!(display_width("\x1b"), "\\u{1b}".len());
    let rows = vec![
        vec!["界".to_owned(), "x".to_owned()],
        vec!["a".to_owned(), "y".to_owned()],
    ];
    let mut output = Vec::new();
    print_table_with_width(&mut output, &["A", "B"], &rows, None).unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "A  | B\n---+--\n界 | x\na  | y\n"
    );
}

#[test]
fn bounds_table_rows_cells_and_terminal_width() {
    let rows = (0..MAX_PRINTED_ROWS + 2)
        .map(|_| vec!["界".repeat(MAX_CELL_WIDTH), "value".to_owned()])
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    print_table_with_width(&mut output, &["Wide", "Value"], &rows, Some(24)).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("…"));
    assert!(output.ends_with("(2 rows omitted)\n"));
    for line in output.lines().take(MAX_PRINTED_ROWS + 2) {
        assert!(UnicodeWidthStr::width(line) <= 24, "{line:?}");
    }

    let headers = ["A", "B", "C", "D", "E", "F"];
    let mut output = Vec::new();
    print_table_with_width(&mut output, &headers, &[vec!["x".to_owned(); 6]], Some(8)).unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("(4 columns omitted)")
    );
}
