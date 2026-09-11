use open_sdbl::{DiagnosticKind, Keyword, Lexer, TokenKind, tokenize};

const KEYWORD_ALIASES: [(Keyword, &str, &str); 57] = [
    (Keyword::Select, "ВЫБРАТЬ", "SELECT"),
    (Keyword::From, "ИЗ", "FROM"),
    (Keyword::Where, "ГДЕ", "WHERE"),
    (Keyword::As, "КАК", "AS"),
    (Keyword::And, "И", "AND"),
    (Keyword::Or, "ИЛИ", "OR"),
    (Keyword::Not, "НЕ", "NOT"),
    (Keyword::In, "В", "IN"),
    (Keyword::Is, "ЕСТЬ", "IS"),
    (Keyword::Null, "NULL", "NULL"),
    (Keyword::True, "ИСТИНА", "TRUE"),
    (Keyword::False, "ЛОЖЬ", "FALSE"),
    (Keyword::Distinct, "РАЗЛИЧНЫЕ", "DISTINCT"),
    (Keyword::Top, "ПЕРВЫЕ", "TOP"),
    (Keyword::Allowed, "РАЗРЕШЕННЫЕ", "ALLOWED"),
    (Keyword::Order, "УПОРЯДОЧИТЬ", "ORDER"),
    (Keyword::By, "ПО", "BY"),
    (Keyword::Group, "СГРУППИРОВАТЬ", "GROUP"),
    (Keyword::Having, "ИМЕЮЩИЕ", "HAVING"),
    (Keyword::Union, "ОБЪЕДИНИТЬ", "UNION"),
    (Keyword::All, "ВСЕ", "ALL"),
    (Keyword::Into, "ПОМЕСТИТЬ", "INTO"),
    (Keyword::Join, "СОЕДИНЕНИЕ", "JOIN"),
    (Keyword::Left, "ЛЕВОЕ", "LEFT"),
    (Keyword::Right, "ПРАВОЕ", "RIGHT"),
    (Keyword::Full, "ПОЛНОЕ", "FULL"),
    (Keyword::Inner, "ВНУТРЕННЕЕ", "INNER"),
    (Keyword::Outer, "ВНЕШНЕЕ", "OUTER"),
    (Keyword::On, "ON", "ON"),
    (Keyword::Case, "ВЫБОР", "CASE"),
    (Keyword::When, "КОГДА", "WHEN"),
    (Keyword::Then, "ТОГДА", "THEN"),
    (Keyword::Else, "ИНАЧЕ", "ELSE"),
    (Keyword::End, "КОНЕЦ", "END"),
    (
        Keyword::RefPresentation,
        "ПРЕДСТАВЛЕНИЕССЫЛКИ",
        "REFPRESENTATION",
    ),
    (Keyword::Presentation, "ПРЕДСТАВЛЕНИЕ", "PRESENTATION"),
    (Keyword::Count, "КОЛИЧЕСТВО", "COUNT"),
    (Keyword::Sum, "СУММА", "SUM"),
    (Keyword::Min, "МИНИМУМ", "MIN"),
    (Keyword::Max, "МАКСИМУМ", "MAX"),
    (Keyword::SliceLast, "СРЕЗПОСЛЕДНИХ", "SLICELAST"),
    (Keyword::SliceFirst, "СРЕЗПЕРВЫХ", "SLICEFIRST"),
    (Keyword::Balance, "ОСТАТКИ", "BALANCE"),
    (Keyword::Turnovers, "ОБОРОТЫ", "TURNOVERS"),
    (Keyword::DateTime, "ДАТАВРЕМЯ", "DATETIME"),
    (Keyword::BeginOfPeriod, "НАЧАЛОПЕРИОДА", "BEGINOFPERIOD"),
    (Keyword::Value, "ЗНАЧЕНИЕ", "VALUE"),
    (Keyword::Uuid, "УНИКАЛЬНЫЙИДЕНТИФИКАТОР", "UUID"),
    (Keyword::Cast, "ВЫРАЗИТЬ", "CAST"),
    (Keyword::IsNullFunction, "ЕСТЬNULL", "ISNULL"),
    (Keyword::Like, "ПОДОБНО", "LIKE"),
    (Keyword::Escape, "СПЕЦСИМВОЛ", "ESCAPE"),
    (Keyword::Add, "ДОБАВИТЬ", "ADD"),
    (Keyword::Drop, "УНИЧТОЖИТЬ", "DROP"),
    (Keyword::Index, "ИНДЕКСИРОВАТЬ", "INDEX"),
    (Keyword::Sets, "НАБОРАМ", "SETS"),
    (Keyword::Unique, "УНИКАЛЬНО", "UNIQUE"),
];

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
fn recognizes_the_complete_bilingual_keyword_table() {
    assert_eq!(KEYWORD_ALIASES.len(), 57);
    for (index, (keyword, russian, english)) in KEYWORD_ALIASES.into_iter().enumerate() {
        assert!(
            KEYWORD_ALIASES[..index]
                .iter()
                .all(|(previous, _, _)| *previous != keyword),
            "duplicate keyword in coverage table: {keyword:?}"
        );
        for spelling in [russian, english] {
            let lowercase = spelling.to_lowercase();
            let source = format!("{spelling} {lowercase}");
            let tokens = tokenize(&source).unwrap();

            assert_eq!(tokens.len(), 2, "{spelling}");
            for token in tokens {
                assert_eq!(token.kind, TokenKind::Keyword(keyword), "{spelling}");
                assert_eq!(
                    token.lexeme,
                    &source[token.span.start..token.span.end],
                    "{spelling}"
                );
            }
        }
    }
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

#[test]
fn iterator_yields_one_diagnostic_and_then_fuses() {
    let mut lexer = Lexer::new("SELECT Code @ FROM");

    assert_eq!(
        lexer.next().unwrap().unwrap().kind,
        TokenKind::Keyword(Keyword::Select)
    );
    assert_eq!(lexer.next().unwrap().unwrap().kind, TokenKind::Identifier);
    let error = lexer.next().unwrap().unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::UnexpectedCharacter('@'));
    assert_eq!((error.offset, error.line, error.column), (12, 1, 13));
    assert!(lexer.next().is_none());
    assert!(lexer.next().is_none());
    assert_eq!(lexer.next_token(), Ok(None));
}

#[test]
fn classifies_every_operator_and_punctuation_lexeme() {
    let source = "= <> < <= > >= + - * / ( ) [ ] , . ;";
    let tokens = tokenize(source).unwrap();
    let expected = [
        (TokenKind::Operator, "="),
        (TokenKind::Operator, "<>"),
        (TokenKind::Operator, "<"),
        (TokenKind::Operator, "<="),
        (TokenKind::Operator, ">"),
        (TokenKind::Operator, ">="),
        (TokenKind::Operator, "+"),
        (TokenKind::Operator, "-"),
        (TokenKind::Operator, "*"),
        (TokenKind::Operator, "/"),
        (TokenKind::Punctuation, "("),
        (TokenKind::Punctuation, ")"),
        (TokenKind::Punctuation, "["),
        (TokenKind::Punctuation, "]"),
        (TokenKind::Punctuation, ","),
        (TokenKind::Punctuation, "."),
        (TokenKind::Punctuation, ";"),
    ];

    assert_eq!(tokens.len(), expected.len());
    for (token, (kind, lexeme)) in tokens.iter().zip(expected) {
        assert_eq!((token.kind, token.lexeme), (kind, lexeme));
    }
}

#[test]
fn tokenizes_decimal_numbers_without_consuming_a_trailing_period() {
    let tokens = tokenize("0 12 12.34 12.").unwrap();
    let actual = tokens
        .iter()
        .map(|token| (token.kind, token.lexeme))
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        [
            (TokenKind::Number, "0"),
            (TokenKind::Number, "12"),
            (TokenKind::Number, "12.34"),
            (TokenKind::Number, "12"),
            (TokenKind::Punctuation, "."),
        ]
    );
}

#[test]
fn handles_empty_input_comments_at_eof_and_crlf_positions() {
    assert!(tokenize("").unwrap().is_empty());
    let mut empty = Lexer::new("");
    assert!(empty.next().is_none());
    assert!(empty.next().is_none());

    let comment = tokenize("// comment at EOF").unwrap();
    assert_eq!(comment.len(), 1);
    assert_eq!(comment[0].kind, TokenKind::Comment);
    assert_eq!(comment[0].lexeme, "// comment at EOF");

    let source = "SELECT\r\nКод";
    let tokens = tokenize(source).unwrap();
    assert_eq!((tokens[1].span.line, tokens[1].span.column), (2, 1));
    assert_eq!((tokens[1].span.start, tokens[1].span.end), (8, 14));
}

#[test]
fn reports_bom_and_other_unexpected_characters_with_positions() {
    for (source, character, offset, line, column) in [
        ("\u{feff}SELECT", '\u{feff}', 0, 1, 1),
        ("SELECT\n  @", '@', 9, 2, 3),
    ] {
        let error = tokenize(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::UnexpectedCharacter(character));
        assert_eq!(
            (error.offset, error.line, error.column),
            (offset, line, column)
        );
    }
}

#[test]
fn every_token_span_round_trips_to_its_lexeme() {
    let source = "ВЫБРАТЬ\r\n  Код, 12.50, \"текст\" // comment";
    let tokens = tokenize(source).unwrap();

    assert!(!tokens.is_empty());
    for token in &tokens {
        assert_eq!(token.lexeme, &source[token.span.start..token.span.end]);
    }
    assert!(
        tokens
            .windows(2)
            .all(|pair| pair[0].span.end <= pair[1].span.start)
    );
}

#[test]
fn recognizes_temporary_table_keywords_bilingually() {
    let tokens =
        tokenize("ДОБАВИТЬ ВТ; УНИЧТОЖИТЬ ВТ; ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код) УНИКАЛЬНО)").unwrap();
    let kinds = tokens.iter().map(|token| token.kind).collect::<Vec<_>>();

    assert_eq!(kinds[0], TokenKind::Keyword(Keyword::Add));
    assert_eq!(kinds[3], TokenKind::Keyword(Keyword::Drop));
    assert_eq!(kinds[6], TokenKind::Keyword(Keyword::Index));
    assert_eq!(kinds[7], TokenKind::Keyword(Keyword::By));
    assert_eq!(kinds[8], TokenKind::Keyword(Keyword::Sets));
    assert!(kinds.contains(&TokenKind::Keyword(Keyword::Unique)));

    let english = tokenize("add drop index by sets unique").unwrap();
    assert_eq!(
        english.iter().map(|token| token.kind).collect::<Vec<_>>(),
        vec![
            TokenKind::Keyword(Keyword::Add),
            TokenKind::Keyword(Keyword::Drop),
            TokenKind::Keyword(Keyword::Index),
            TokenKind::Keyword(Keyword::By),
            TokenKind::Keyword(Keyword::Sets),
            TokenKind::Keyword(Keyword::Unique),
        ]
    );
}
