//! Real queries of the 1C demo configuration, compiled against a fixture
//! of its metadata. The recorded result of every query is either the
//! generated PostgreSQL text or the diagnostic the compiler reports, so a
//! change in either shows up as a diff.

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use open_sdbl::metadata::{FieldId, MetadataSnapshot, StandardFieldId};
use open_sdbl::query::{
    CompileOptions, ParameterDate, ParameterValue, PostgresBackend, PresentationExpression,
    PresentationPlan, QueryCompiler, QueryParameter, SessionParameters, queryable_fields,
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

/// Reads one optional string field, absent from most corpus entries.
fn optional_json_field(line: &str, name: &str) -> Option<String> {
    let key = format!("\"{name}\":");
    let start = line.find(&key)? + key.len();
    Some(unescape(line[start..].trim_start()))
}

/// One recorded corpus entry: the query text, the values its parameters
/// take, and whether the pruned fixture carries the metadata it names.
struct CorpusEntry {
    source: String,
    text: String,
    /// Parameter bindings as `name` → recorded literal, in the spelling
    /// [`parameter_value`] reads.
    parameters: Vec<(String, String)>,
    /// The pruned fixture does not carry the metadata this query names, so
    /// it says nothing about the compiler.
    beyond_fixture: bool,
}

impl CorpusEntry {
    fn read(line: &str) -> Self {
        Self {
            source: json_field(line, "source"),
            text: json_field(line, "text"),
            parameters: read_parameters(line),
            beyond_fixture: optional_json_field(line, "expect").as_deref() == Some("fixture"),
        }
    }

    fn write(&self) -> String {
        let mut out = format!(
            "{{\"source\":{},\"text\":{}",
            escape(&self.source),
            escape(&self.text)
        );
        if !self.parameters.is_empty() {
            out.push_str(",\"params\":{");
            for (index, (name, value)) in self.parameters.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&format!("{}:{}", escape(name), escape(value)));
            }
            out.push('}');
        }
        if self.beyond_fixture {
            out.push_str(",\"expect\":\"fixture\"");
        }
        out.push('}');
        out
    }
}

/// Reads the `params` object: a flat map of parameter name to recorded
/// literal.
fn read_parameters(line: &str) -> Vec<(String, String)> {
    let Some(start) = line.find("\"params\":") else {
        return Vec::new();
    };
    let body = line[start + "\"params\":".len()..].trim_start();
    let Some(body) = body.strip_prefix('{') else {
        return Vec::new();
    };
    let mut parameters = Vec::new();
    let mut rest = body;
    loop {
        rest = rest.trim_start();
        if rest.starts_with('}') || rest.is_empty() {
            break;
        }
        let name = unescape(rest);
        rest = skip_string(rest);
        rest = rest.trim_start().strip_prefix(':').unwrap_or(rest);
        let value = unescape(rest.trim_start());
        rest = skip_string(rest.trim_start());
        parameters.push((name, value));
        rest = rest.trim_start().strip_prefix(',').unwrap_or(rest);
    }
    parameters
}

/// Steps over one JSON string, honoring its escapes.
fn skip_string(text: &str) -> &str {
    let mut characters = text.char_indices();
    let Some((_, '"')) = characters.next() else {
        return text;
    };
    while let Some((index, character)) = characters.next() {
        match character {
            '\\' => {
                characters.next();
            }
            '"' => return &text[index + 1..],
            _ => {}
        }
    }
    ""
}

/// Reads one recorded parameter literal. The letter says the type the use
/// of the parameter requires, which an untyped `NULL` cannot satisfy:
/// `D20260101` and `D20260101235959` are dates, `S…` a string, `N…` a
/// number, `B0`/`B1` a boolean, and anything else `NULL`.
fn parameter_value(recorded: &str) -> ParameterValue {
    let (tag, rest) = recorded.split_at(recorded.len().min(1));
    match tag {
        "D" => {
            let digits = |from: usize, to: usize| {
                rest.get(from..to)
                    .and_then(|part| part.parse::<u16>().ok())
                    .unwrap_or(0)
            };
            let part = |from: usize, to: usize| u8::try_from(digits(from, to)).unwrap_or(0);
            ParameterValue::Date(
                ParameterDate::new(
                    digits(0, 4),
                    part(4, 6).max(1),
                    part(6, 8).max(1),
                    part(8, 10),
                    part(10, 12),
                    part(12, 14),
                )
                .expect("recorded corpus date must be valid"),
            )
        }
        "S" => ParameterValue::String(rest.to_owned()),
        "N" => ParameterValue::Number {
            unscaled: rest.parse().unwrap_or(0),
            scale: 0,
        },
        "B" => ParameterValue::Boolean(rest == "1"),
        _ => ParameterValue::Null,
    }
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

/// A named parameter takes the value the entry records for it, `NULL`
/// when it records none, and the data separators are switched off. A use
/// that requires a type — a register slice takes a date — would refuse an
/// untyped `NULL`, so those entries carry a recorded value.
fn options(entry: &CorpusEntry) -> (Vec<QueryParameter>, SessionParameters) {
    let names = tokenize(&entry.text)
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
        .map(|name| {
            let recorded = entry
                .parameters
                .iter()
                .find(|(recorded, _)| recorded == name)
                .map_or(ParameterValue::Null, |(_, value)| parameter_value(value));
            QueryParameter::new(name, recorded)
        })
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

/// The recorded text of one compiled query: the main statement, then the
/// statement of every tabular section it projects, so a change in either
/// shows up as a diff.
fn record(compiled: &open_sdbl::query::CompiledQuery) -> String {
    let mut out = compiled.sql.clone();
    for nested in &compiled.nested {
        out.push_str(&format!("\n-- nested {} --\n{}", nested.label, nested.sql));
    }
    out
}

/// Compiles one corpus query. A query that asks for a reference
/// presentation needs the two-phase protocol — the application answers the
/// request with a plan — so the harness stands in for the application and
/// presents every requested object by its description, falling back to its
/// code and then to a fixed literal. Without this the recorded result would
/// say the harness omitted a plan, not what the compiler does.
fn compile_corpus_query(
    snapshot: &MetadataSnapshot,
    query: &str,
    parameters: &[QueryParameter],
    session: &SessionParameters,
) -> Result<String, open_sdbl::query::QueryDiagnostic> {
    let options = CompileOptions::new()
        .parameters(parameters)
        .session(session);
    let direct = QueryCompiler::new(snapshot, PostgresBackend).compile_with(query, &options);
    if !matches!(
        &direct,
        Err(error) if error.kind() == open_sdbl::query::QueryDiagnosticKind::PresentationPlan
    ) {
        return direct.map(|compiled| record(&compiled));
    }
    let prepared = QueryCompiler::new(snapshot, PostgresBackend).prepare(query)?;
    let plans = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| stand_in_plan(snapshot, target.object))
        .collect::<Vec<_>>();
    prepared
        .compile_with(snapshot, &options.presentations(&plans))
        .map(|compiled| record(&compiled))
}

/// The plan the harness answers with: the object's description, else its
/// code, else a literal that names no column.
fn stand_in_plan(
    snapshot: &MetadataSnapshot,
    object: open_sdbl::metadata::ObjectId,
) -> PresentationPlan {
    let has = |name: &str| {
        snapshot
            .object_by_id(object)
            .and_then(|object| queryable_fields(snapshot, object).ok())
            .is_some_and(|fields| {
                fields
                    .iter()
                    .any(|field| field.schema_name.eq_ignore_ascii_case(name))
            })
    };
    let field = if has("Description") {
        Some(FieldId::Standard(StandardFieldId::Description))
    } else if has("Code") {
        Some(FieldId::Standard(StandardFieldId::Code))
    } else {
        None
    };
    match field {
        Some(field) => PresentationPlan {
            object,
            fields: vec![field],
            expression: PresentationExpression::Field(field),
        },
        None => PresentationPlan {
            object,
            fields: Vec::new(),
            expression: PresentationExpression::Literal("<представление>".to_owned()),
        },
    }
}

/// Whether a recorded text is a query at all. The configuration stores
/// interface captions that begin with the word `Выбрать` — "Выбрать
/// пользователя", "Выбрать версию для восстановления…" — which the
/// extraction cannot tell from a query by its first word, and which the
/// parser reads as a projection of one field with an alias.
///
/// A text is taken as a query when it names something through a dot, which
/// every query over metadata does and no caption does, or when it compiles
/// — that keeps the source-less technical queries the configuration really
/// issues, such as `ВЫБРАТЬ NULL КАК Ссылка`. A source-less query the
/// compiler cannot yet compile would be dropped, which shows up as a
/// shrinking corpus.
fn is_query(snapshot: &MetadataSnapshot, entry: &CorpusEntry) -> bool {
    let names_something = tokenize(&entry.text).is_ok_and(|tokens| {
        tokens.windows(3).any(|window| {
            window[1].kind == TokenKind::Punctuation
                && window[1].lexeme == "."
                && window[0].kind == TokenKind::Identifier
                && window[2].kind == TokenKind::Identifier
        })
    });
    if names_something {
        return true;
    }
    let (parameters, session) = options(entry);
    compile_corpus_query(snapshot, &entry.text, &parameters, &session).is_ok()
}

/// Rewrites `corpus.jsonl` and `expected.jsonl` from the current compiler.
/// Run it after a change that the corpus test reports, and commit the
/// diff: `cargo test -p open-sdbl --test query_corpus -- --ignored`.
#[test]
#[ignore = "maintenance: rewrites the recorded results"]
fn rerecord_the_demo_corpus() {
    let root = fixture();
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let mut recorded = String::new();
    let mut expected = String::new();
    let mut compiled = 0usize;
    let mut refused = 0usize;
    for line in corpus.lines().filter(|line| !line.trim().is_empty()) {
        let entry = CorpusEntry::read(line);
        if !is_query(&snapshot, &entry) {
            refused += 1;
            continue;
        }
        let (parameters, session) = options(&entry);
        let outcome = match compile_corpus_query(&snapshot, &entry.text, &parameters, &session) {
            Ok(sql) => {
                compiled += 1;
                sql
            }
            Err(error) => format!("!{:?}: {}", error.kind(), error.message()),
        };
        recorded.push_str(&entry.write());
        recorded.push('\n');
        expected.push_str(&escape(&outcome));
        expected.push('\n');
    }
    std::fs::write(root.join("corpus.jsonl"), recorded).unwrap();
    std::fs::write(root.join("expected.jsonl"), expected).unwrap();
    println!(
        "recorded {compiled} compiling queries of {}, refused {refused} texts that are not queries",
        corpus
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
            - refused
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
        let entry = CorpusEntry::read(line);
        let query = entry.text.clone();
        let (parameters, session) = options(&entry);
        let Err(error) = compile_corpus_query(&snapshot, &query, &parameters, &session) else {
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
    let queries: Vec<CorpusEntry> = corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(CorpusEntry::read)
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
    let mut beyond_fixture = 0usize;
    for (index, (entry, expected)) in queries.iter().zip(&recorded).enumerate() {
        let (parameters, session) = options(entry);
        let outcome = match compile_corpus_query(&snapshot, &entry.text, &parameters, &session) {
            Ok(sql) => {
                compiled += 1;
                sql
            }
            Err(error) => format!("!{:?}: {}", error.kind(), error.message()),
        };
        if entry.beyond_fixture {
            beyond_fixture += 1;
            assert!(
                outcome.starts_with('!') && outcome.contains("was not found"),
                "query {index} is marked as beyond the fixture but resolves: {outcome}"
            );
        }
        if &outcome != expected {
            let head: String = entry.text.chars().take(70).collect();
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
    // Entries whose metadata the pruned fixture does not carry say nothing
    // about the compiler, so they are counted out of the denominator.
    assert_eq!(compiled, 374, "queries that compile");
    assert_eq!(
        queries.len() - beyond_fixture,
        381,
        "queries the fixture can answer for"
    );
}

/// Writes the corpus compiled for SQL Server into `MSSQL_CORPUS_OUT`, so
/// that a T-SQL server can be asked whether it accepts every statement the
/// compiler produces. The PostgreSQL side of the same check runs against a
/// live base as well; neither is part of the workspace test run:
/// `MSSQL_CORPUS_OUT=/tmp/corpus.sql cargo test -p open-sdbl --test
/// query_corpus -- --ignored writes_the_corpus_for_sql_server`.
#[test]
#[ignore = "maintenance: writes the corpus compiled for SQL Server"]
fn writes_the_corpus_for_sql_server() {
    let Ok(out) = std::env::var("MSSQL_CORPUS_OUT") else {
        return;
    };
    let root = fixture();
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let mut written = String::new();
    let mut compiled = 0usize;
    for (index, line) in corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let entry = CorpusEntry::read(line);
        let (parameters, session) = options(&entry);
        let options = CompileOptions::new()
            .parameters(&parameters)
            .session(&session);
        let backend = open_sdbl::query::MsSqlBackend::new(2000).unwrap();
        let Ok(result) = QueryCompiler::new(&snapshot, backend).compile_with(&entry.text, &options)
        else {
            continue;
        };
        compiled += 1;
        written.push_str(&format!("-- query {}\n{}\n--;\n", index + 1, result.sql));
    }
    std::fs::write(&out, written).unwrap();
    println!("wrote {compiled} statements to {out}");
}
