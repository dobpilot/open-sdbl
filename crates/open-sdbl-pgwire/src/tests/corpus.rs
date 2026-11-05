//! Every stored SDBL query through the authenticated gateway and real psql.
use super::protocol::{Frame, error_of, logged_in};
use super::support::corpus::{CorpusEntry, escape, options, unescape};
use super::*;
use open_sdbl::metadata::{Guid, MetadataSnapshot};
use open_sdbl::{TokenKind, tokenize};
use ring::digest::{Context, SHA256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}
fn baseline(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/sdbl-corpus/{name}.tsv"))
}
fn fixtures() -> Vec<&'static str> {
    let selected = std::env::var("CORPUS_FIXTURE").unwrap_or_else(|_| "demo,unf,buh".into());
    let selected = selected.split(',').map(str::trim).collect::<Vec<_>>();
    assert!(
        selected.iter().all(|n| ["demo", "unf", "buh"].contains(n)),
        "unknown CORPUS_FIXTURE"
    );
    ["demo", "unf", "buh"]
        .into_iter()
        .filter(|n| selected.contains(n))
        .collect()
}
fn load(name: &str) -> (MetadataSnapshot, Vec<CorpusEntry>, Vec<String>) {
    let path = root().join(name);
    let entries = std::fs::read_to_string(path.join("corpus.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(CorpusEntry::read)
        .collect::<Vec<_>>();
    assert_eq!(
        entries.len(),
        match name {
            "demo" => 382,
            "unf" => 708,
            "buh" => 636,
            _ => unreachable!(),
        }
    );
    let expected = std::fs::read_to_string(path.join("expected.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(unescape)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), expected.len());
    (support::demo_resolved_at(&path).snapshot, entries, expected)
}
fn harness(snapshot: MetadataSnapshot, first: &CorpusEntry) -> Harness {
    let base = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, true);
    // A broad test role still goes through the real, fail-closed field mapper.
    rights.objects.clear();
    rights.templates.clear();
    rights.set_for_new_objects = true;
    rights.set_for_attributes_by_default = true;
    access.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights);
    let (_, operator, _) = options(first, &snapshot);
    Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        operator,
        responder: Arc::new(|_, _| Ok(vec![])),
        ..Options::default()
    })
}

/// psql Simple Query cannot bind SDBL named parameters. Render the corpus's
/// scalar values as SDBL literals using lexer spans, never string replacement
/// inside literals/comments. Keep table values unbound and report their refusal.
fn text(entry: &CorpusEntry, snapshot: &MetadataSnapshot) -> String {
    let (parameters, _, _) = options(entry, snapshot);
    let Ok(tokens) = tokenize(&entry.text) else {
        return entry.text.clone();
    };
    let mut result = String::new();
    let mut offset = 0;
    for token in tokens.iter().filter(|t| t.kind == TokenKind::Parameter) {
        let name = token.lexeme.trim_start_matches('&');
        let parameter = parameters
            .iter()
            .find(|p| p.name().to_lowercase() == name.to_lowercase())
            .unwrap();
        let value = match parameter.value() {
            ParameterValue::Null => "NULL".into(),
            ParameterValue::Date(d) => format!(
                "ДАТАВРЕМЯ({},{},{},{},{},{})",
                d.year(),
                d.month(),
                d.day(),
                d.hour(),
                d.minute(),
                d.second()
            ),
            ParameterValue::String(s) => format!("\"{}\"", s.replace('"', "\"\"")),
            ParameterValue::Boolean(v) => if *v { "ИСТИНА" } else { "ЛОЖЬ" }.into(),
            ParameterValue::Number { unscaled, scale: 0 } => unscaled.to_string(),
            _ => token.lexeme.into(),
        };
        result.push_str(&entry.text[offset..token.span.start]);
        result.push_str(&value);
        offset = token.span.end;
    }
    result.push_str(&entry.text[offset..]);
    result
}
fn hash(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut context = Context::new(&SHA256);
    for part in parts {
        let bytes = part.as_ref();
        context.update(&(bytes.len() as u64).to_be_bytes());
        context.update(bytes);
    }
    context
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    state: String,
    sql_count: usize,
    sql_hash: String,
    descriptions: String,
    message: String,
}
impl Outcome {
    fn read(line: &str) -> Self {
        let c = line.split('\t').collect::<Vec<_>>();
        assert_eq!(c.len(), 9, "baseline columns");
        Self {
            state: c[3].into(),
            sql_count: c[4].parse().unwrap(),
            sql_hash: c[5].into(),
            descriptions: c[6].into(),
            message: unescape(c[8]),
        }
    }
    fn record(&self, index: usize, entry: &CorpusEntry, expected: &str) -> String {
        let compiler = if entry.beyond_fixture {
            "fixture"
        } else if expected.starts_with('!') {
            "diagnostic"
        } else {
            "compiled"
        };
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            index + 1,
            entry.source,
            hash([entry.text.as_bytes()]),
            self.state,
            self.sql_count,
            self.sql_hash,
            self.descriptions,
            compiler,
            escape(&self.message)
        )
    }
}
fn recorded(name: &str, entries: &[CorpusEntry]) -> Vec<Outcome> {
    let text =
        std::fs::read_to_string(baseline(name)).expect("record with PGWIRE_UPDATE_CORPUS=1 first");
    let result = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .enumerate()
        .map(|(i, l)| {
            assert_eq!(
                l.split('\t').next().unwrap().parse::<usize>().unwrap(),
                i + 1
            );
            let columns = l.split('\t').collect::<Vec<_>>();
            assert_eq!(columns[1], entries[i].source, "corpus source identity");
            assert_eq!(
                columns[2],
                hash([entries[i].text.as_bytes()]),
                "corpus text fingerprint"
            );
            Outcome::read(l)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        result.len(),
        entries.len(),
        "every corpus entry has a gateway baseline"
    );
    result
}
fn outcome(frames: &[Frame], statements: &[String]) -> Outcome {
    assert_eq!(
        frames.last().map(|f| f.0),
        Some(b'Z'),
        "gateway must recover through ReadyForQuery"
    );
    let state = if frames.iter().any(|f| f.0 == b'E') {
        error_of(frames)[&b'C'].clone()
    } else {
        assert!(
            frames.iter().any(|f| f.0 == b'C'),
            "success has CommandComplete"
        );
        "OK".into()
    };
    assert!(
        !["XX000", "08P01", "08006"].contains(&state.as_str()),
        "unexpected gateway/internal error: {frames:?}"
    );
    Outcome {
        state,
        sql_count: statements.len(),
        sql_hash: hash(statements.iter().map(String::as_bytes)),
        descriptions: hash(
            frames
                .iter()
                .filter(|f| f.0 == b'T')
                .map(|f| f.1.as_slice()),
        ),
        message: if frames.iter().any(|f| f.0 == b'E') {
            error_of(frames)[&b'M'].clone()
        } else {
            String::new()
        },
    }
}
fn summarize(name: &str, results: &[Outcome]) {
    let mut counts = BTreeMap::new();
    for r in results {
        *counts.entry(r.state.as_str()).or_insert(0_usize) += 1;
    }
    eprintln!("{name}: {} attempted: {counts:?}", results.len());
}

#[tokio::test]
#[ignore = "full 1726-query protocol corpus replay; reads native metadata packs"]
async fn sdbl_corpus_through_gateway() {
    let update = std::env::var("PGWIRE_UPDATE_CORPUS").as_deref() == Ok("1");
    for name in fixtures() {
        let (snapshot, entries, core) = load(name);
        let texts = entries
            .iter()
            .map(|entry| text(entry, &snapshot))
            .collect::<Vec<_>>();
        let expected = (!update).then(|| recorded(name, &entries));
        let harness = harness(snapshot, &entries[0]);
        let mut raw = logged_in(&harness).await;
        let mut results = vec![];
        let mut lines = String::from(
            "# index\tsource\tsource-sha256\tSQLSTATE\tDB-statements\tSQL-sha256\tdescription-sha256\tcompiler\tmessage-json\n",
        );
        for (i, ((entry, text), core)) in entries.iter().zip(&texts).zip(&core).enumerate() {
            let start = harness.statements().len();
            let frames = raw.query(text).await;
            let statements = harness.statements();
            let actual = outcome(&frames, &statements[start..]);
            assert!(
                !statements[start..].iter().any(|s| s.trim() == text.trim()),
                "raw client text reached the DB: {name}:{}",
                i + 1
            );
            if let Some(expected) = &expected {
                assert_eq!(
                    actual,
                    expected[i],
                    "{name}:{} {}\n{text}\n{:?}",
                    i + 1,
                    entry.source,
                    frames
                        .iter()
                        .filter(|f| f.0 == b'E')
                        .map(|f| protocol::fields(&f.1))
                        .collect::<Vec<_>>()
                );
            }
            lines.push_str(&actual.record(i, entry, core));
            results.push(actual);
            let reset = raw.query("DISCARD ALL").await;
            assert!(!reset.iter().any(|f| f.0 == b'E'));
        }
        summarize(name, &results);
        if update {
            std::fs::create_dir_all(baseline(name).parent().unwrap()).unwrap();
            std::fs::write(baseline(name), lines).unwrap();
        }
        harness.stop();
    }
}

#[tokio::test]
#[ignore = "requires psql and loopback sockets; replays all 1726 SDBL entries"]
async fn psql_sdbl_corpus() {
    assert_ne!(
        std::env::var("PGWIRE_UPDATE_CORPUS").as_deref(),
        Ok("1"),
        "psql verification never records baselines"
    );
    for name in fixtures() {
        let (snapshot, entries, _) = load(name);
        let texts = entries
            .iter()
            .map(|entry| text(entry, &snapshot))
            .collect::<Vec<_>>();
        let expected = recorded(name, &entries);
        let harness = harness(snapshot, &entries[0]);
        let server = psql::Server::start(&harness).await;
        for (i, ((entry, text), expected)) in entries.iter().zip(&texts).zip(&expected).enumerate()
        {
            let start = harness.statements().len();
            let output = server
                .run(
                    &["-q", "-A", "-t", "-v", "VERBOSITY=sqlstate", "-c", text],
                    None,
                )
                .await;
            let actual = psql::state(&output);
            assert_eq!(
                actual,
                expected.state,
                "{name}:{} {}\n{text}\n{}",
                i + 1,
                entry.source,
                String::from_utf8_lossy(&output.stderr)
            );
            let statements = harness.statements();
            assert_eq!(
                statements.len() - start,
                expected.sql_count,
                "{name}:{} statement count",
                i + 1
            );
            assert_eq!(
                hash(statements[start..].iter().map(String::as_bytes)),
                expected.sql_hash,
                "{name}:{} generated SQL",
                i + 1
            );
            if (i + 1) % 100 == 0 {
                eprintln!("psql {name}: {}/{}", i + 1, entries.len());
            }
        }
        summarize(name, &expected);
        harness.stop();
    }
}

#[test]
fn scalar_binding_rendering_does_not_touch_strings_or_comments() {
    let entry = CorpusEntry {
        source: "test".into(),
        text: "ВЫБРАТЬ &Дата, \"&Дата\" // &Дата\n".into(),
        parameters: vec![("Дата".into(), "D20260102030405".into())],
        beyond_fixture: false,
    };
    assert_eq!(
        text(&entry, &fixture::snapshot()),
        "ВЫБРАТЬ ДАТАВРЕМЯ(2026,1,2,3,4,5), \"&Дата\" // &Дата\n"
    );
}

#[test]
fn corpus_baselines_cover_every_source_entry() {
    for name in ["demo", "unf", "buh"] {
        let entries = std::fs::read_to_string(root().join(name).join("corpus.jsonl"))
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(CorpusEntry::read)
            .collect::<Vec<_>>();
        let results = recorded(name, &entries);
        assert!(results.iter().any(|r| r.state == "OK"));
        assert!(results.iter().any(|r| r.state == "42501"));
    }
}
