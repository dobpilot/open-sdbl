//! Tests of the console `terminal` module.



use super::*;

#[test]
fn recognizes_multiline_termination_outside_strings_and_comments() {
    assert!(!statement_is_complete("ВЫБРАТЬ Код\n"));
    assert!(statement_is_complete("ВЫБРАТЬ Код\nИЗ Справочник.Тест;\n"));
    assert!(!statement_is_complete("ВЫБРАТЬ \"text;\"\n"));
    assert!(statement_is_complete("ВЫБРАТЬ Код; // done\n"));
    assert!(!statement_is_complete("ВЫБРАТЬ Код // ;\n"));
}

#[test]
fn validates_cyrillic_bytes_without_terminating_the_reader() {
    assert_eq!(
        decode_input_line("select Код Из Справочник.Договоры;\n".as_bytes()).unwrap(),
        "select Код Из Справочник.Договоры;\n"
    );

    let mut damaged = "select Код".as_bytes().to_vec();
    damaged.pop();
    damaged.extend_from_slice(b";\n");
    assert_eq!(decode_input_line(&damaged), Err(damaged.len() - 3));
}

#[tokio::test]
async fn bounds_non_interactive_input_lines_and_drains_the_remainder() {
    let source = b"123456789\nnext\n";
    let mut input = tokio::io::BufReader::new(&source[..]);
    let mut line = Vec::new();
    assert_eq!(
        read_bounded_line(&mut input, &mut line, 4).await.unwrap(),
        BoundedLine::TooLong
    );
    assert_eq!(line.len(), 5);
    line.clear();
    assert_eq!(
        read_bounded_line(&mut input, &mut line, 8).await.unwrap(),
        BoundedLine::Read(5)
    );
    assert_eq!(line, b"next\n");
}

#[test]
fn footer_fits_the_last_terminal_row() {
    assert_eq!(footer_text(1), "");
    assert_eq!(footer_text(4), "...");
    assert!(footer_text(24).len() < 24);
    assert_eq!(footer_text(200), super::COMMAND_HINT);
}
