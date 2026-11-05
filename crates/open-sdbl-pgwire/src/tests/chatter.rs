use super::*;

fn set(name: &str, value: Option<&str>) -> Chatter {
    Chatter::Set {
        name: name.to_owned(),
        value: value.map(str::to_owned),
    }
}

#[test]
fn driver_chatter_is_recognized() {
    let cases = [
        ("", vec![Chatter::Empty]),
        ("  ;  ", vec![Chatter::Empty]),
        ("-- only a comment", vec![Chatter::Empty]),
        ("/* block */ // sdbl", vec![Chatter::Empty]),
        (
            "SET application_name = 'DBeaver 24.1'",
            vec![set("application_name", Some("DBeaver 24.1"))],
        ),
        (
            "set extra_float_digits to 3",
            vec![set("extra_float_digits", Some("3"))],
        ),
        (
            "SET SESSION statement_timeout = DEFAULT",
            vec![set("statement_timeout", None)],
        ),
        (
            "SET client_encoding = 'UTF8'",
            vec![set("client_encoding", Some("UTF8"))],
        ),
        (
            "SET NAMES 'LATIN1'",
            vec![set("client_encoding", Some("LATIN1"))],
        ),
        ("SET TIME ZONE 'UTC'", vec![set("timezone", Some("UTC"))]),
        (
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE",
            vec![Chatter::SetReadWrite],
        ),
        ("RESET ALL", vec![Chatter::Reset(None)]),
        (
            "reset application_name",
            vec![Chatter::Reset(Some("application_name".to_owned()))],
        ),
        (
            "SHOW search_path",
            vec![Chatter::Show("search_path".to_owned())],
        ),
        (
            "SHOW TRANSACTION ISOLATION LEVEL",
            vec![Chatter::Show("transaction_isolation".to_owned())],
        ),
        (
            "BEGIN",
            vec![Chatter::Begin {
                read_write: false,
                tag: "BEGIN",
            }],
        ),
        (
            "begin transaction read only;",
            vec![Chatter::Begin {
                read_write: false,
                tag: "BEGIN",
            }],
        ),
        (
            "START TRANSACTION READ ONLY",
            vec![Chatter::Begin {
                read_write: false,
                tag: "START TRANSACTION",
            }],
        ),
        (
            "START TRANSACTION READ WRITE",
            vec![Chatter::Begin {
                read_write: true,
                tag: "START TRANSACTION",
            }],
        ),
        ("COMMIT", vec![Chatter::Commit]),
        ("END", vec![Chatter::Commit]),
        ("commit work", vec![Chatter::Commit]),
        ("ROLLBACK", vec![Chatter::Rollback]),
        ("ABORT", vec![Chatter::Rollback]),
        ("DISCARD ALL", vec![Chatter::DiscardAll]),
        ("SELECT version()", vec![Chatter::Version]),
        ("select  version ( )", vec![Chatter::Version]),
        ("SELECT current_schema()", vec![Chatter::CurrentSchema]),
        ("SELECT 1", vec![Chatter::SelectOne]),
        (
            "SET extra_float_digits = 3; SET application_name = 'x;y'",
            vec![
                set("extra_float_digits", Some("3")),
                set("application_name", Some("x;y")),
            ],
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(classify(text), Some(expected), "{text}");
    }
}

#[test]
fn everything_else_is_sdbl() {
    for text in [
        "ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура",
        "SELECT * FROM _Reference42",
        "SELECT 2",
        "SELECT version(), 1",
        "BEGIN; ВЫБРАТЬ 1",
        "COMMIT AND CHAIN",
        "DISCARD PLANS",
        "SHOW",
        "SET",
        "SET x",
        "begin isolation level read uncommitted maybe",
        "ВЫБРАТЬ 1 // SET x = 1",
    ] {
        assert_eq!(classify(text), None, "{text}");
    }
}

#[test]
fn utf8_spellings() {
    for name in ["UTF8", "utf-8", "Unicode", " utf_8 "] {
        assert!(is_utf8(name), "{name}");
    }
    for name in ["LATIN1", "WIN1251", "SQL_ASCII"] {
        assert!(!is_utf8(name), "{name}");
    }
}

#[test]
fn datagrip_bootstrap_whitelist_uses_tokens_and_complete_statements() {
    let epoch = "select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time";
    let ssl = "select ssl from pg_stat_ssl where pid = pg_backend_pid()";
    assert_eq!(classify(epoch), Some(vec![Chatter::StartupTime]));
    assert_eq!(
        classify(&format!("/* probe */ {ssl}; -- end")),
        Some(vec![Chatter::SslStatus])
    );
    for sql in [
        format!("{epoch}; DROP TABLE t"),
        epoch.replace("'UTC'", "'Europe/Moscow'"),
        ssl.replace("pg_backend_pid()", "123"),
        ssl.replace("select ssl", "select ssl,pid"),
        format!("{epoch} /* unclosed"),
    ] {
        assert_eq!(classify(&sql), None, "{sql}");
    }
}

#[test]
fn string_probes_preserve_sql_quoting_and_reject_other_shapes() {
    for (sql, value, name) in [
        ("SELECT 'keep alive'", "keep alive", "?column?"),
        ("select ''", "", "?column?"),
        (
            "/* outer /* nested */ */ SELECT 'I''m; -- /* alive' AS \"KeepAlive\"; -- tail",
            "I'm; -- /* alive",
            "KeepAlive",
        ),
        (r"SELECT E'line\nnext' AS probe", "line\nnext", "probe"),
        (
            "SELECT $probe$Привет; 'alive'$probe$",
            "Привет; 'alive'",
            "?column?",
        ),
    ] {
        assert_eq!(
            classify(sql),
            Some(vec![Chatter::SelectText {
                value: value.into(),
                name: name.into()
            }]),
            "{sql}"
        );
    }
    for sql in [
        "SELECT 'keep alive' FROM _Reference42",
        "SELECT 'keep alive' FROM pg_class",
        "SELECT 'keep alive' WHERE true",
        "SELECT 'keep alive' LIMIT 0",
        "SELECT 'keep alive' ORDER BY 1",
        "SELECT 'keep alive' GROUP BY 1",
        "SELECT 'keep alive' HAVING true",
        "SELECT DISTINCT 'keep alive'",
        "SELECT 'a' UNION SELECT 'b'",
        "SELECT 'a', 'b'",
        "SELECT 'a' || 'b'",
        "SELECT 'a'::text",
        "SELECT 'a'; SELECT 'b'",
        "SELECT 'a'; DROP TABLE t",
        "SELECT $1",
        "SELECT 'unterminated",
        "SELECT 'alive' /* unterminated",
        "ВЫБРАТЬ \"keep alive\"",
    ] {
        assert_eq!(classify(sql), None, "{sql}");
    }
}
