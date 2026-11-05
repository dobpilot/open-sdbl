//! Real queries of the 1C demo configuration, compiled against a fixture
//! of its metadata. The recorded result of every query is either the
//! generated PostgreSQL text or the diagnostic the compiler reports, so a
//! change in either shows up as a diff.

mod support;
use support::corpus::{CorpusEntry, escape, options, unescape};

use std::path::PathBuf;

use open_sdbl::metadata::{FieldId, MetadataSnapshot, StandardFieldId};
use open_sdbl::query::{
    ColumnKind, CompileOptions, ParameterValue, PostgresBackend, PresentationExpression,
    PresentationPlan, QueryCompiler, QueryParameter, SessionParameters, queryable_fields,
};
use open_sdbl::{TokenKind, tokenize};

/// The corpus fixtures to run: every known one that exists, or the one
/// `CORPUS_FIXTURE` names (`demo`, `unf`, `buh`).
fn fixtures() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let names = std::env::var("CORPUS_FIXTURE").unwrap_or_else(|_| "demo,unf,buh".to_owned());
    names
        .split(',')
        .map(|name| root.join(name.trim()))
        .filter(|path| path.join("corpus.jsonl").is_file())
        .collect()
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
    predefined: &[QueryParameter],
) -> Result<String, open_sdbl::query::QueryDiagnostic> {
    let options = CompileOptions::new()
        .parameters(parameters)
        .session(session);
    if !predefined.is_empty() {
        // The temporary tables of other batches, defined from their typed
        // placeholders before the batch runs.
        let mut manager = open_sdbl::query::TempTablesManager::new();
        let compiler = QueryCompiler::new(snapshot, PostgresBackend);
        for parameter in predefined {
            let ParameterValue::Table { columns, .. } = parameter.value() else {
                continue;
            };
            let name = parameter.name().trim_start_matches("__vt_");
            let projection = if columns.is_empty() {
                "1 КАК __stub".to_owned()
            } else {
                columns
                    .iter()
                    .map(|column| format!("Т.{0} КАК {0}", column.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let definition = format!(
                "ВЫБРАТЬ {projection} ПОМЕСТИТЬ {name} ИЗ &{} КАК Т;",
                parameter.name()
            );
            let one = [parameter.clone()];
            let definition_options = CompileOptions::new().parameters(&one).session(session);
            compiler.compile_batch(&definition, &definition_options, &mut manager)?;
        }
        let batch = match compiler.compile_batch(query, &options, &mut manager) {
            Err(error)
                if error.kind() == open_sdbl::query::QueryDiagnosticKind::PresentationPlan =>
            {
                let prepared = compiler.prepare_with(query, &manager)?;
                let plans = prepared
                    .presentation_request()
                    .targets
                    .iter()
                    .map(|target| stand_in_plan(snapshot, target.object))
                    .collect::<Vec<_>>();
                prepared.compile_batch(snapshot, &options.presentations(&plans), &mut manager)?
            }
            other => other?,
        };
        return match batch {
            Some(compiled) => Ok(record(&compiled)),
            None => {
                // A batch that drops every table it defines answers nothing.
                let Some(table) = last_defined_table(query) else {
                    return Ok("-- the batch defines and drops its tables".to_owned());
                };
                let count_options = CompileOptions::new().session(session);
                compiler
                    .compile_batch(
                        &format!("ВЫБРАТЬ КОЛИЧЕСТВО(*) КАК Н ИЗ {table} КАК Т;"),
                        &count_options,
                        &mut manager,
                    )
                    .map(|compiled| record(&compiled.expect("a final statement answers rows")))
            }
        };
    }
    let direct = QueryCompiler::new(snapshot, PostgresBackend).compile_with(query, &options);
    // A batch that ends with `ПОМЕСТИТЬ` answers no rows; the platform
    // runs it for the table it leaves, so the record is the count over
    // that table with the batch's definitions in front.
    if let Err(error) = &direct
        && error.kind() == open_sdbl::query::QueryDiagnosticKind::TemporaryTable
        && error.message().contains("returns no rows")
    {
        let mut manager = open_sdbl::query::TempTablesManager::new();
        let compiler = QueryCompiler::new(snapshot, PostgresBackend);
        compiler.compile_batch(query, &options, &mut manager)?;
        // A batch that drops every table it defines answers nothing.
        let Some(table) = last_defined_table(query) else {
            return Ok("-- the batch defines and drops its tables".to_owned());
        };
        // The count names no parameter of the batch.
        let count_options = CompileOptions::new().session(session);
        return compiler
            .compile_batch(
                &format!("ВЫБРАТЬ КОЛИЧЕСТВО(*) КАК Н ИЗ {table} КАК Т;"),
                &count_options,
                &mut manager,
            )
            .map(|compiled| record(&compiled.expect("a final statement answers rows")));
    }
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

/// The name after the last `ПОМЕСТИТЬ`/`INTO` of a batch.
fn last_defined_table(query: &str) -> Option<String> {
    let tokens = tokenize(query).ok()?;
    let mut name = None;
    for pair in tokens.windows(2) {
        if pair[0].kind == TokenKind::Keyword(open_sdbl::Keyword::Into)
            && pair[1].kind == TokenKind::Identifier
        {
            name = Some(pair[1].lexeme.to_owned());
        }
        // A table the batch drops again is not there to count.
        if pair[0].kind == TokenKind::Keyword(open_sdbl::Keyword::Drop)
            && pair[1].kind == TokenKind::Identifier
            && name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(pair[1].lexeme))
        {
            name = None;
        }
    }
    name
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
    let (parameters, session, predefined) = options(entry, snapshot);
    compile_corpus_query(snapshot, &entry.text, &parameters, &session, &predefined).is_ok()
}

/// Writes `<Вид.Объект>\t<Поле>\t<вид>` for every field of the fixture
/// `CORPUS_FIXTURE` names into `KINDS_OUT`, which
/// `tools/corpus/bind_tables.py --kinds` reads to type the placeholder
/// columns a query compares with those fields. The kind is spelled the
/// way the `T…` parameter tag spells it: `ЧИСЛО`, `СТРОКА`, `ДАТА`,
/// `БУЛЕВО`, `ЛЮБАЯССЫЛКА` or `Вид.Объект[|…]`; a composite field with
/// primitive members has no single kind and is left out.
#[test]
#[ignore]
fn dump_field_kinds() {
    let Ok(out) = std::env::var("KINDS_OUT") else {
        return;
    };
    let name = std::env::var("CORPUS_FIXTURE").unwrap_or_else(|_| "demo".to_owned());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"));
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let kind_name = |kind: open_sdbl::metadata::MetadataKind| {
        use open_sdbl::metadata::MetadataKind as K;
        match kind {
            K::Catalog => Some("Справочник"),
            K::Document => Some("Документ"),
            K::DocumentJournal => Some("ЖурналДокументов"),
            K::Enumeration => Some("Перечисление"),
            K::InformationRegister => Some("РегистрСведений"),
            K::AccumulationRegister => Some("РегистрНакопления"),
            K::AccountingRegister => Some("РегистрБухгалтерии"),
            K::CalculationRegister => Some("РегистрРасчета"),
            K::ChartOfCharacteristicTypes => Some("ПланВидовХарактеристик"),
            K::ChartOfCalculationTypes => Some("ПланВидовРасчета"),
            K::ChartOfAccounts => Some("ПланСчетов"),
            K::ExchangePlan => Some("ПланОбмена"),
            K::BusinessProcess => Some("БизнесПроцесс"),
            K::Task => Some("Задача"),
            _ => None,
        }
    };
    let object_name = |object: &open_sdbl::metadata::MetadataObject| -> Option<String> {
        let name = object.name.as_deref()?;
        match object.owner.and_then(|owner| snapshot.object_by_id(owner)) {
            // A tabular section is named under its owner.
            Some(owner) => Some(format!(
                "{}.{}.{name}",
                kind_name(owner.kind?)?,
                owner.name.as_deref()?
            )),
            None => Some(format!("{}.{name}", kind_name(object.kind?)?)),
        }
    };
    let targets_tag = |ids: &[open_sdbl::metadata::ObjectId]| -> String {
        let names = ids
            .iter()
            .filter_map(|id| snapshot.object_by_id(*id))
            .filter_map(object_name)
            .collect::<Vec<_>>();
        if names.is_empty() {
            "ЛЮБАЯССЫЛКА".to_owned()
        } else {
            names.join("|")
        }
    };
    let mut lines = Vec::new();
    for object in snapshot.objects() {
        let Some(name) = object_name(object) else {
            continue;
        };
        let Ok(fields) = queryable_fields(&snapshot, object) else {
            continue;
        };
        for field in fields {
            let tag = match field.columns.as_slice() {
                [column] => match &column.kind {
                    ColumnKind::Number { .. } => "ЧИСЛО".to_owned(),
                    ColumnKind::String { .. } => "СТРОКА".to_owned(),
                    ColumnKind::DateTime => "ДАТА".to_owned(),
                    ColumnKind::Boolean => "БУЛЕВО".to_owned(),
                    ColumnKind::Reference { targets, .. } => targets_tag(targets),
                    _ => continue,
                },
                columns
                    if columns.iter().all(|column| {
                        column.is_reference_type_member()
                            || column.is_reference_value_member()
                            || column.physical_name.to_ascii_lowercase().ends_with("_type")
                    }) =>
                {
                    let names = field
                        .reference_targets
                        .iter()
                        .filter_map(|table| {
                            snapshot.objects().iter().find(|object| {
                                object
                                    .physical_table
                                    .as_deref()
                                    .is_some_and(|physical| physical.eq_ignore_ascii_case(table))
                            })
                        })
                        .filter_map(object_name)
                        .collect::<Vec<_>>();
                    if names.is_empty() {
                        "ЛЮБАЯССЫЛКА".to_owned()
                    } else {
                        names.join("|")
                    }
                }
                _ => continue,
            };
            // Every name the compiler accepts for the field: the metadata
            // name, the schema name and their English spellings.
            let mut spellings = vec![field.name.clone()];
            spellings.extend(field.aliases.iter().cloned());
            spellings.sort();
            spellings.dedup();
            for spelling in spellings {
                lines.push(format!("{name}\t{spelling}\t{tag}"));
            }
        }
    }
    lines.sort();
    std::fs::write(&out, lines.join("\n") + "\n").unwrap();
    println!("kinds: {} fields of {name}", lines.len());
}

/// Rewrites `corpus.jsonl` and `expected.jsonl` from the current compiler.
/// Run it after a change that the corpus test reports, and commit the
/// diff: `cargo test -p open-sdbl --test query_corpus -- --ignored`.
#[test]
#[ignore = "maintenance: rewrites the recorded results"]
fn rerecord_the_demo_corpus() {
    for root in fixtures() {
        rerecord(&root);
    }
}

fn rerecord(root: &std::path::Path) {
    let snapshot = support::demo_resolved_at(root).snapshot;
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
        let (parameters, session, predefined) = options(&entry, &snapshot);
        let outcome = match compile_corpus_query(
            &snapshot,
            &entry.text,
            &parameters,
            &session,
            &predefined,
        ) {
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
        "{}: recorded {compiled} compiling queries of {}, refused {refused} texts that are not queries",
        root.display(),
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
    for root in fixtures() {
        locate(&root);
    }
}

fn locate(root: &std::path::Path) {
    let snapshot = support::demo_resolved_at(root).snapshot;
    let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
    let filter = std::env::var("GAP").unwrap_or_default();
    for (index, line) in corpus
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let entry = CorpusEntry::read(line);
        let query = entry.text.clone();
        let (parameters, session, predefined) = options(&entry, &snapshot);
        let Err(error) =
            compile_corpus_query(&snapshot, &query, &parameters, &session, &predefined)
        else {
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

#[test]
fn compiles_the_demo_corpus_as_recorded() {
    let fixtures = fixtures();
    assert!(!fixtures.is_empty(), "no corpus fixture found");
    for root in fixtures {
        compiles_as_recorded(&root);
    }
}

fn compiles_as_recorded(root: &std::path::Path) {
    let snapshot = support::demo_resolved_at(root).snapshot;
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
        let (parameters, session, predefined) = options(entry, &snapshot);
        let outcome = match compile_corpus_query(
            &snapshot,
            &entry.text,
            &parameters,
            &session,
            &predefined,
        ) {
            Ok(sql) => {
                compiled += 1;
                sql
            }
            Err(error) => format!("!{:?}: {}", error.kind(), error.message()),
        };
        if entry.beyond_fixture {
            beyond_fixture += 1;
            // Such an entry may stop earlier than the missing object, on
            // a gap of the compiler; what it must not do is compile.
            assert!(
                outcome.starts_with('!'),
                "query {index} is marked as beyond the fixture but compiles: {outcome}"
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
    if let Some((expected_compiled, expected_total)) = recorded_share(root) {
        assert_eq!(compiled, expected_compiled, "queries that compile");
        assert_eq!(
            queries.len() - beyond_fixture,
            expected_total,
            "queries the fixture can answer for"
        );
    }
}

/// The recorded share of one fixture: how many queries compile, out of
/// how many the fixture can answer for. `None` for a fixture whose share
/// is not pinned yet.
fn recorded_share(root: &std::path::Path) -> Option<(usize, usize)> {
    match root.file_name()?.to_str()? {
        "demo" => Some((377, 381)),
        "unf" => Some((674, 688)),
        "buh" => Some((613, 622)),
        _ => None,
    }
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
    let mut written = String::new();
    let mut compiled = 0usize;
    for root in fixtures() {
        let snapshot = support::demo_resolved_at(&root).snapshot;
        let corpus = std::fs::read_to_string(root.join("corpus.jsonl")).unwrap();
        for (index, line) in corpus
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
        {
            let entry = CorpusEntry::read(line);
            let (parameters, session, _predefined) = options(&entry, &snapshot);
            let options = CompileOptions::new()
                .parameters(&parameters)
                .session(&session);
            let backend = open_sdbl::query::MsSqlBackend::new(2000).unwrap();
            let Ok(result) =
                QueryCompiler::new(&snapshot, backend).compile_with(&entry.text, &options)
            else {
                continue;
            };
            compiled += 1;
            written.push_str(&format!(
                "-- {} query {}\n{}\n--;\n",
                root.display(),
                index + 1,
                result.sql
            ));
        }
    }
    std::fs::write(&out, written).unwrap();
    println!("wrote {compiled} statements to {out}");
}
