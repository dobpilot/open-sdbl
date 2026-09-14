//! Real queries of the 1C demo configuration, compiled against a fixture
//! of its metadata. The recorded result of every query is either the
//! generated PostgreSQL text or the diagnostic the compiler reports, so a
//! change in either shows up as a diff.

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use open_sdbl::query::{
    CompileOptions, ParameterValue, PostgresBackend, QueryCompiler, QueryParameter,
    SessionParameters,
};
use open_sdbl::{TokenKind, tokenize};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo")
}

/// Reads one string field of a flat JSON object of the corpus file.
fn json_field(line: &str, name: &str) -> String {
    let key = format!("\"{name}\":");
    let start = line.find(&key).expect("field") + key.len();
    unescape(line[start..].trim_start())
}

fn unescape(text: &str) -> String {
    let mut characters = text.strip_prefix('"').expect("string").chars();
    let mut out = String::new();
    while let Some(character) = characters.next() {
        match character {
            '"' => break,
            '\\' => match characters.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let hex: String = characters.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        out.push(char::from_u32(code).unwrap_or('?'));
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            other => out.push(other),
        }
    }
    out
}

/// Every named parameter is bound to `NULL` and the data separators are
/// switched off, so the corpus compiles without inventing values.
fn options(text: &str) -> (Vec<QueryParameter>, SessionParameters) {
    let names = tokenize(text)
        .map(|tokens| {
            tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Parameter)
                .map(|token| token.lexeme.trim_start_matches('&').to_owned())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let parameters = names
        .iter()
        .map(|name| QueryParameter::new(name, ParameterValue::Null))
        .collect::<Vec<_>>();
    let mut session = SessionParameters::new();
    for (name, value) in [
        (
            "ОбластьДанныхЗначение",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        ("ОбластьДанныхИспользование", ParameterValue::Boolean(false)),
        (
            "ОбластьДанныхОсновныеДанные",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        (
            "ИспользованиеРазделителяСеанса",
            ParameterValue::Boolean(false),
        ),
        (
            "ЗначениеРазделителя",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        ("ИспользованиеРазделителя", ParameterValue::Boolean(false)),
    ] {
        session.set(QueryParameter::new(name, value));
    }
    (parameters, session)
}

/// Rewrites `expected.jsonl` from the current compiler. Run it after a
/// change that the corpus test reports, and commit the diff:
/// `cargo test -p open-sdbl --test query_corpus -- --ignored`.
#[test]
#[ignore = "maintenance: rewrites the recorded results"]
fn rerecord_the_demo_corpus() {
    let root = fixture();
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let mut expected = String::new();
    let mut compiled = 0usize;
    for line in corpus.lines().filter(|line| !line.trim().is_empty()) {
        let query = json_field(line, "text");
        let (parameters, session) = options(&query);
        let outcome = match QueryCompiler::new(&snapshot, PostgresBackend).compile_with(
            &query,
            &CompileOptions::new()
                .parameters(&parameters)
                .session(&session),
        ) {
            Ok(result) => {
                compiled += 1;
                result.sql
            }
            Err(error) => format!("!{:?}: {}", error.kind(), error.message()),
        };
        expected.push_str(&escape(&outcome));
        expected.push('\n');
    }
    std::fs::write(root.join("expected.jsonl"), expected).unwrap();
    println!(
        "recorded {compiled} compiling queries of {}",
        corpus.lines().count()
    );
}

/// Prints where the corpus queries whose diagnostic contains `$GAP` stop,
/// with the surrounding text, so a gap can be read without guessing:
/// `GAP="expected field name" cargo test -p open-sdbl --test query_corpus \
/// -- --ignored locate`.
#[test]
#[ignore = "maintenance: reports where the corpus stops"]
fn locate_the_corpus_gaps() {
    let root = fixture();
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let filter = std::env::var("GAP").unwrap_or_default();
    for (index, line) in corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let query = json_field(line, "text");
        let (parameters, session) = options(&query);
        let Err(error) = QueryCompiler::new(&snapshot, PostgresBackend).compile_with(
            &query,
            &CompileOptions::new()
                .parameters(&parameters)
                .session(&session),
        ) else {
            continue;
        };
        if !error.message().contains(&filter) {
            continue;
        }
        let offset = error.offset().min(query.len());
        let start = query[..offset]
            .char_indices()
            .rev()
            .nth(90)
            .map_or(0, |(index, _)| index);
        let end = query[offset..]
            .char_indices()
            .nth(60)
            .map_or(query.len(), |(index, _)| offset + index);
        println!(
            "#{index} {}\n   …{}<<HERE>>{}…",
            error.message(),
            query[start..offset].replace('\n', " "),
            query[offset..end].replace('\n', " "),
        );
    }
}

/// Writes a JSON string the way the corpus files spell one.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[test]
fn compiles_the_demo_corpus_as_recorded() {
    let root = fixture();
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let expected = std::fs::read_to_string(root.join("expected.jsonl")).unwrap();
    let queries: Vec<String> = corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| json_field(line, "text"))
        .collect();
    let recorded: Vec<String> = expected
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(unescape)
        .collect();
    assert_eq!(
        queries.len(),
        recorded.len(),
        "the corpus and the recorded results must stay aligned"
    );

    let mut differences = Vec::new();
    let mut compiled = 0usize;
    for (index, (query, expected)) in queries.iter().zip(&recorded).enumerate() {
        let (parameters, session) = options(query);
        let outcome = match QueryCompiler::new(&snapshot, PostgresBackend).compile_with(
            query,
            &CompileOptions::new()
                .parameters(&parameters)
                .session(&session),
        ) {
            Ok(result) => {
                compiled += 1;
                result.sql
            }
            Err(error) => format!("!{:?}: {}", error.kind(), error.message()),
        };
        if &outcome != expected {
            let head: String = query.chars().take(70).collect();
            differences.push(format!(
                "query {index} ({}…):\n  expected: {}\n  actual:   {}",
                head.replace('\n', " "),
                expected.chars().take(160).collect::<String>(),
                outcome.chars().take(160).collect::<String>()
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "{} of {} corpus queries changed:\n{}",
        differences.len(),
        queries.len(),
        differences
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // The share that compiles is recorded so an improvement is visible.
    assert_eq!(compiled, 293, "queries that compile");
}
