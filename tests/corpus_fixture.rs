//! Maintenance: cuts the full metadata dump of a base down to the fixture a
//! query corpus needs, the way `tests/fixtures/demo` was built. The dump
//! comes from `tools/corpus/fetch_base.py`; the fixture keeps the objects
//! the corpus names, two steps of the reference targets of their fields,
//! and every common attribute, plus the tables of those objects.
//!
//! ```console
//! CORPUS_FULL=/path/to/dump CORPUS_OUT=tests/fixtures/unf \
//!   cargo test -p open-sdbl --test corpus_fixture -- --ignored
//! ```

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::{TokenKind, tokenize};

/// The metadata kind an SDBL source word names, in the spellings the
/// language accepts.
fn kind_of(word: &str) -> Option<MetadataKind> {
    Some(match word.to_lowercase().as_str() {
        "справочник" | "catalog" | "reference" => MetadataKind::Catalog,
        "документ" | "document" => MetadataKind::Document,
        "журналдокументов" | "documentjournal" => MetadataKind::DocumentJournal,
        "перечисление" | "enum" | "enumeration" => MetadataKind::Enumeration,
        "регистрсведений" | "informationregister" | "inforg" => {
            MetadataKind::InformationRegister
        }
        "регистрнакопления" | "accumulationregister" | "accumrg" => {
            MetadataKind::AccumulationRegister
        }
        "регистрбухгалтерии" | "accountingregister" | "accrg" => {
            MetadataKind::AccountingRegister
        }
        "регистррасчета" | "calculationregister" | "crg" => {
            MetadataKind::CalculationRegister
        }
        "планвидовхарактеристик" | "chartofcharacteristictypes" | "chrc" => {
            MetadataKind::ChartOfCharacteristicTypes
        }
        "планвидоврасчета" | "chartofcalculationtypes" | "ckinds" => {
            MetadataKind::ChartOfCalculationTypes
        }
        "плансчетов" | "chartofaccounts" | "acc" => MetadataKind::ChartOfAccounts,
        "константа" | "constant" | "const" => MetadataKind::Constant,
        "планобмена" | "exchangeplan" | "node" => MetadataKind::ExchangePlan,
        "бизнеспроцесс" | "businessprocess" | "bpr" => MetadataKind::BusinessProcess,
        "задача" | "task" => MetadataKind::Task,
        "последовательность" | "sequence" => MetadataKind::Sequence,
        _ => return None,
    })
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

fn json_field(line: &str, name: &str) -> String {
    let key = format!("\"{name}\":");
    let start = line.find(&key).expect("field") + key.len();
    unescape(line[start..].trim_start())
}

/// The `Вид.Имя` pairs a query names in a source position, as `(kind,
/// name)`: after `ИЗ`, a join, a comma, an opening parenthesis, `КАК` (a
/// cast target) or `ССЫЛКА`, or at the very start. A pair after a dot or
/// an alias is a field path (`Т.Документ.Дата`) whose first word only
/// looks like a kind.
fn named_objects(text: &str) -> Vec<(MetadataKind, String)> {
    let Ok(tokens) = tokenize(text) else {
        return Vec::new();
    };
    // A source alias spelled like a kind (`… КАК РегистрНакопления`) makes
    // every `РегистрНакопления.Поле` a field path, not an object name.
    let aliases: BTreeSet<String> = tokens
        .windows(2)
        .filter(|window| {
            matches!(window[0].lexeme.to_uppercase().as_str(), "КАК" | "AS")
                && window[1].kind == TokenKind::Identifier
        })
        .map(|window| window[1].lexeme.to_lowercase())
        .collect();
    (0..tokens.len().saturating_sub(2))
        .filter(|&index| {
            let window = &tokens[index..index + 3];
            window[1].kind == TokenKind::Punctuation
                && window[1].lexeme == "."
                && window[0].kind == TokenKind::Identifier
                && window[2].kind == TokenKind::Identifier
                && !aliases.contains(&window[0].lexeme.to_lowercase())
                && (index == 0
                    || matches!(
                        tokens[index - 1].lexeme.to_uppercase().as_str(),
                        "ИЗ" | "FROM"
                            | "СОЕДИНЕНИЕ"
                            | "JOIN"
                            | ","
                            | "("
                            | "КАК"
                            | "AS"
                            | "ССЫЛКА"
                            | "REFS"
                    ))
        })
        .filter_map(|index| {
            Some((
                kind_of(tokens[index].lexeme)?,
                tokens[index + 2].lexeme.to_owned(),
            ))
        })
        .collect()
}

fn lower_table(name: &str) -> String {
    let name = name.strip_prefix('_').unwrap_or(name);
    format!("_{}", name.to_lowercase())
}

/// Every physical table an object owns: its main table, the service tables
/// resolved to it, the DBNames entries that share its GUID (totals, changes,
/// extra dimensions), and the inline tables (tabular sections) of those.
fn tables_of(snapshot: &MetadataSnapshot, id: ObjectId) -> BTreeSet<String> {
    let mut tables = BTreeSet::new();
    let Some(object) = snapshot.object_by_id(id) else {
        return tables;
    };
    if let Some(table) = &object.physical_table {
        tables.insert(lower_table(table));
    }
    for other in snapshot.objects() {
        if other.owner == Some(id) {
            if let Some(table) = &other.physical_table {
                tables.insert(lower_table(table));
            }
        }
    }
    for entry in snapshot.db_names().entries() {
        if entry.guid == object.guid {
            tables.insert(lower_table(&format!("{}{}", entry.alias, entry.number)));
        }
    }
    let owned: Vec<String> = tables.iter().cloned().collect();
    for table in &snapshot.schema().tables {
        if let Some(owner) = &table.owner {
            if owned.contains(&lower_table(owner)) {
                tables.insert(lower_table(&table.name));
            }
        }
    }
    tables
}

/// The objects the fields of one object point at: reference targets named
/// by SchemaStorage and reference types named by the Config descriptors.
fn targets_of(snapshot: &MetadataSnapshot, id: ObjectId) -> BTreeSet<ObjectId> {
    let mut targets = BTreeSet::new();
    let Some(object) = snapshot.object_by_id(id) else {
        return targets;
    };
    for table in tables_of(snapshot, id) {
        let Some(schema) = snapshot.schema_table(&table) else {
            continue;
        };
        for column in &schema.columns {
            for column_type in &column.types {
                if let Some(target) = &column_type.reference_target {
                    if let Ok(target) = snapshot.object_id_by_physical_table(&lower_table(target)) {
                        targets.insert(target);
                    }
                }
            }
        }
    }
    let by_reference_type: BTreeMap<String, ObjectId> = snapshot
        .descriptors()
        .iter()
        .filter_map(|descriptor| {
            let reference_type = descriptor.object_reference_type.as_ref()?;
            Some((
                reference_type.to_string().to_lowercase(),
                ObjectId::from(&descriptor.object_guid),
            ))
        })
        .collect();
    for descriptor in snapshot.descriptors() {
        if descriptor.resource_guid != object.guid {
            continue;
        }
        for reference_type in &descriptor.reference_types {
            if let Some(target) = by_reference_type.get(&reference_type.to_string().to_lowercase())
            {
                if snapshot.object_by_id(*target).is_some() {
                    targets.insert(*target);
                }
            }
        }
    }
    targets
}

/// Splits the serialized SchemaStorage `{0,{N,{table},{table},…}}` into
/// the texts of its table declarations, honoring quoted strings.
fn schema_tables(text: &str) -> (String, Vec<String>) {
    let bytes = text.as_bytes();
    let first = text.find('{').expect("outer brace");
    let second = first + 1 + text[first + 1..].find('{').expect("list brace");
    let prefix = text[..=second].to_owned();
    let mut elements = Vec::new();
    let mut index = second + 1;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut start = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    continue;
                }
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' => {
                    if depth == 0 {
                        start = Some(index);
                    }
                    depth += 1;
                }
                b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        elements.push(text[start.take().unwrap()..=index].to_owned());
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    (prefix, elements)
}

fn element_name(element: &str) -> Option<String> {
    let start = element.find('"')? + 1;
    let end = start + element[start..].find('"')?;
    Some(element[start..end].to_owned())
}

#[test]
#[ignore = "maintenance: builds a fixture from a full base dump"]
fn prune_the_full_dump_to_a_fixture() {
    let full = PathBuf::from(std::env::var("CORPUS_FULL").expect("CORPUS_FULL=<dump directory>"));
    let out = PathBuf::from(std::env::var("CORPUS_OUT").expect("CORPUS_OUT=<fixture directory>"));
    let hops: usize = std::env::var("CORPUS_HOPS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2);
    let resolved = support::demo_resolved_at(&full);
    let snapshot = &resolved.snapshot;
    println!(
        "full base: {} objects, {} fields, {} findings",
        snapshot.objects().len(),
        snapshot.fields().len(),
        resolved.report.findings().len()
    );

    let corpus = std::fs::read_to_string(full.join("corpus.jsonl")).unwrap();
    // The parameter bindings recorded in the fixture so far survive a
    // rebuild: they are keyed by the query text.
    let bound: BTreeMap<String, String> = std::fs::read_to_string(out.join("corpus.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("\"params\":"))
        .map(|line| {
            let start = line.find("\"params\":").unwrap();
            let end = start + line[start..].find('}').unwrap() + 1;
            (json_field(line, "text"), line[start..end].to_owned())
        })
        .collect();
    let mut kept: BTreeSet<ObjectId> = BTreeSet::new();
    let mut beyond: BTreeSet<String> = BTreeSet::new();
    let mut recorded = String::new();
    for line in corpus.lines().filter(|line| !line.trim().is_empty()) {
        let text = json_field(line, "text");
        let mut resolves = true;
        for (kind, name) in named_objects(&text) {
            match snapshot.object_id(kind, &name) {
                Ok(id) => {
                    kept.insert(id);
                }
                Err(_) => {
                    beyond.insert(format!("{kind:?}.{name}"));
                    resolves = false;
                }
            }
        }
        let line = line.trim_end();
        let mut line = line.strip_suffix('}').unwrap_or(line).to_owned();
        if !line.contains("\"params\":") {
            if let Some(params) = bound.get(&text) {
                line.push(',');
                line.push_str(params);
            }
        }
        if !resolves && !line.contains("\"expect\":") {
            line.push_str(",\"expect\":\"fixture\"");
        }
        recorded.push_str(&line);
        recorded.push_str("}\n");
    }
    println!(
        "named by the corpus: {} objects; unresolved: {beyond:?}",
        kept.len()
    );

    let mut frontier: BTreeSet<ObjectId> = kept.clone();
    for hop in 1..=hops {
        let mut next = BTreeSet::new();
        for id in &frontier {
            for target in targets_of(snapshot, *id) {
                if kept.insert(target) {
                    next.insert(target);
                }
            }
        }
        println!("hop {hop}: +{} objects, {} total", next.len(), kept.len());
        frontier = next;
    }

    let mut guids: BTreeSet<String> = kept
        .iter()
        .filter_map(|id| snapshot.object_by_id(*id))
        .map(|object| object.guid.to_string().to_lowercase())
        .collect();
    let mut tables: BTreeSet<String> = BTreeSet::new();
    for id in &kept {
        tables.extend(tables_of(snapshot, *id));
    }
    let common: BTreeSet<String> = snapshot
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.alias == "Fld")
        .map(|entry| entry.guid.to_string().to_lowercase())
        .collect();

    std::fs::create_dir_all(&out).unwrap();
    std::fs::copy(full.join("db_names.deflate"), out.join("db_names.deflate")).unwrap();

    // Config pack: the descriptors of the kept objects and every common
    // attribute (a bare resource whose GUID is a DBNames `Fld` entry).
    let pack = std::fs::read(full.join("config.pack")).unwrap();
    let mut pruned = Vec::new();
    let mut offset = 0usize;
    let mut resources = 0usize;
    let mut common_kept = 0usize;
    while offset < pack.len() {
        let newline = offset
            + pack[offset..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap();
        let header = std::str::from_utf8(&pack[offset..newline]).unwrap();
        let (resource, length) = header.split_once('\t').unwrap();
        let length: usize = length.parse().unwrap();
        let end = newline + 1 + length;
        let base = resource.split('.').next().unwrap().to_lowercase();
        let is_common = !resource.contains('.') && common.contains(&base);
        if guids.contains(&base) || is_common {
            pruned.extend_from_slice(&pack[offset..end]);
            resources += 1;
            common_kept += usize::from(is_common && !guids.contains(&base));
            guids.insert(base);
        }
        offset = end;
    }
    std::fs::write(out.join("config.pack"), pruned).unwrap();

    let schema = std::fs::read_to_string(full.join("schema_storage.txt")).unwrap();
    let (prefix, elements) = schema_tables(&schema);
    let kept_elements: Vec<&String> = elements
        .iter()
        .filter(|element| {
            element_name(element).is_some_and(|name| tables.contains(&lower_table(&name)))
        })
        .collect();
    let list_open = prefix.rfind('{').unwrap();
    let mut trimmed = format!("{}{{{},\n", &prefix[..list_open], kept_elements.len());
    trimmed.push_str(
        &kept_elements
            .iter()
            .map(|element| element.as_str())
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    trimmed.push_str("}}");
    std::fs::write(out.join("schema_storage.txt"), trimmed).unwrap();

    let live = std::fs::read_to_string(full.join("live_columns.tsv")).unwrap();
    let live_kept: String = live
        .lines()
        .filter(|line| {
            line.split('\t')
                .next()
                .is_some_and(|table| tables.contains(&table.to_lowercase()))
        })
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(out.join("live_columns.tsv"), live_kept).unwrap();
    std::fs::write(out.join("corpus.jsonl"), recorded).unwrap();

    println!(
        "fixture {}: {} objects, {resources} Config resources ({common_kept} common attributes), {} schema tables of {}, {} live tables",
        out.display(),
        kept.len(),
        kept_elements.len(),
        elements.len(),
        tables.len()
    );
    let check = support::demo_resolved_at(&out);
    println!(
        "fixture resolves: {} objects, {} fields, {} findings",
        check.objects().len(),
        check.fields().len(),
        check.report.findings().len()
    );
}
