use open_sdbl::{DiagnosticKind, Keyword, TokenKind, tokenize};

#[test]
fn tokenizes_a_representative_query_with_positions() {
    let source =
        "// comment\nВЫБРАТЬ ПЕРВЫЕ 10 Код, \"Иван\"\"ов\" ИЗ Справочник.Города ГДЕ Код >= &МинКод";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].kind, TokenKind::Comment);
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::Select));
    assert_eq!(tokens[1].span.line, 2);
    assert_eq!(tokens[1].span.column, 1);
    assert_eq!(tokens[2].kind, TokenKind::Keyword(Keyword::Top));
    assert_eq!(tokens[3].kind, TokenKind::Number);
    assert_eq!(tokens[6].kind, TokenKind::String);
    assert_eq!(tokens.last().unwrap().kind, TokenKind::Parameter);
    assert_eq!(tokens.last().unwrap().lexeme, "&МинКод");
}

#[test]
fn recognizes_russian_and_english_keywords_case_insensitively() {
    let tokens = tokenize("ВЫБРАТЬ выбрать SELECT select").unwrap();

    assert!(
        tokens
            .iter()
            .all(|token| token.kind == TokenKind::Keyword(Keyword::Select))
    );
}

#[test]
fn tokenizes_hexadecimal_binary_literals_as_one_token() {
    let source = "Version > 0x00000000000007D6 И Probe = 0XCAFE";
    let tokens = tokenize(source).unwrap();
    let binary = tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Binary)
        .collect::<Vec<_>>();

    assert_eq!(binary.len(), 2);
    assert_eq!(binary[0].lexeme, "0x00000000000007D6");
    assert_eq!(binary[1].lexeme, "0XCAFE");
    assert_eq!(
        &source[binary[0].span.start..binary[0].span.end],
        binary[0].lexeme
    );
}

#[test]
fn rejects_malformed_hexadecimal_binary_literals() {
    for source in ["0x", "0x0", "0x0G", "0xGG", "0xCAFEtail"] {
        let error = tokenize(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::InvalidBinaryLiteral, "{source}");
        assert_eq!((error.line, error.column), (1, 1), "{source}");
    }
}

#[test]
fn recognizes_count_bilingually() {
    let tokens = tokenize("count Количество КОЛИЧЕСТВО COUNT").unwrap();
    assert!(
        tokens
            .iter()
            .all(|token| token.kind == TokenKind::Keyword(Keyword::Count))
    );
}

#[test]
fn recognizes_sum_min_and_max_bilingually() {
    let tokens = tokenize("sum Сумма min Минимум max Максимум").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Sum));
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::Sum));
    assert_eq!(tokens[2].kind, TokenKind::Keyword(Keyword::Min));
    assert_eq!(tokens[3].kind, TokenKind::Keyword(Keyword::Min));
    assert_eq!(tokens[4].kind, TokenKind::Keyword(Keyword::Max));
    assert_eq!(tokens[5].kind, TokenKind::Keyword(Keyword::Max));
}

#[test]
fn recognizes_slice_last_bilingually() {
    let tokens = tokenize("СрезПоследних SliceLast").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::SliceLast));
    assert_eq!(tokens[0].lexeme, "СрезПоследних");
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::SliceLast));
    assert_eq!(tokens[1].lexeme, "SliceLast");
}

#[test]
fn recognizes_slice_first_bilingually() {
    let tokens = tokenize("СрезПервых SliceFirst").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::SliceFirst));
    assert_eq!(tokens[0].lexeme, "СрезПервых");
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::SliceFirst));
    assert_eq!(tokens[1].lexeme, "SliceFirst");
}

#[test]
fn recognizes_accumulation_virtual_tables_bilingually() {
    let tokens = tokenize("Остатки Balance Обороты Turnovers").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Balance));
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::Balance));
    assert_eq!(tokens[2].kind, TokenKind::Keyword(Keyword::Turnovers));
    assert_eq!(tokens[3].kind, TokenKind::Keyword(Keyword::Turnovers));
}

#[test]
fn recognizes_date_functions_bilingually() {
    let tokens = tokenize("ДАТАВРЕМЯ DATETIME НАЧАЛОПЕРИОДА BEGINOFPERIOD").unwrap();

    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::DateTime));
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::DateTime));
    assert_eq!(tokens[2].kind, TokenKind::Keyword(Keyword::BeginOfPeriod));
    assert_eq!(tokens[3].kind, TokenKind::Keyword(Keyword::BeginOfPeriod));
}

#[test]
fn recognizes_value_function_bilingually() {
    let tokens = tokenize("ЗНАЧЕНИЕ VALUE").unwrap();

    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Value));
    assert_eq!(tokens[1].kind, TokenKind::Keyword(Keyword::Value));
}

#[test]
fn reports_the_opening_quote_of_an_unterminated_string() {
    let error = tokenize("\n  \"text").unwrap_err();

    assert_eq!(error.kind, DiagnosticKind::UnterminatedString);
    assert_eq!((error.line, error.column), (2, 3));
}

#[test]
fn rejects_a_parameter_without_a_name() {
    let error = tokenize("ГДЕ Код = &").unwrap_err();

    assert_eq!(error.kind, DiagnosticKind::ExpectedParameterName);
    assert_eq!(error.column, 11);
}

#[test]
fn byte_spans_preserve_multibyte_source_text() {
    let source = "Код X";
    let tokens = tokenize(source).unwrap();

    assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "Код");
    assert_eq!((tokens[1].span.start, tokens[1].span.column), (7, 5));
}
