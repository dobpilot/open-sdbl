//! Corpus JSONL, recorded parameter values and operator session bindings shared by tests.
#![allow(dead_code)]
use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    ColumnKind, ParameterColumn, ParameterDate, ParameterValue, QueryParameter, SessionParameters,
    find_metadata_object,
};
use open_sdbl::{TokenKind, tokenize};
use std::collections::BTreeSet;

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
pub(crate) struct CorpusEntry {
    pub(crate) source: String,
    pub(crate) text: String,
    /// Parameter bindings as `name` → recorded literal, in the spelling
    /// [`parameter_value`] reads.
    pub(crate) parameters: Vec<(String, String)>,
    /// The pruned fixture does not carry the metadata this query names, so
    /// it says nothing about the compiler.
    pub(crate) beyond_fixture: bool,
}

impl CorpusEntry {
    pub(crate) fn read(line: &str) -> Self {
        Self {
            source: json_field(line, "source"),
            text: json_field(line, "text"),
            parameters: read_parameters(line),
            beyond_fixture: optional_json_field(line, "expect").as_deref() == Some("fixture"),
        }
    }

    pub(crate) fn write(&self) -> String {
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
/// The kind a recorded column declares.
fn column_kind(text: &str, snapshot: &MetadataSnapshot) -> ColumnKind {
    match text.to_uppercase().as_str() {
        "ЧИСЛО" | "NUMBER" => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        "СТРОКА" | "STRING" => ColumnKind::String { length: None },
        "ДАТА" | "DATE" => ColumnKind::DateTime,
        "БУЛЕВО" | "BOOLEAN" => ColumnKind::Boolean,
        "ЛЮБАЯССЫЛКА" | "ANYREF" => ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        },
        _ => {
            let targets = text
                .split('|')
                .filter_map(|qualified| find_metadata_object(snapshot, qualified).ok())
                .map(|object| open_sdbl::metadata::ObjectId::from(&object.guid))
                .collect::<Vec<_>>();
            ColumnKind::Reference {
                runtime_typed: targets.len() != 1,
                targets,
            }
        }
    }
}

pub(crate) fn parameter_value(recorded: &str, snapshot: &MetadataSnapshot) -> ParameterValue {
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
        // `T<колонка>:<вид>,…`: a value table with those typed columns
        // and no rows — the corpus knows what a query reads, not the data.
        // The kind is `ЧИСЛО`, `СТРОКА`, `ДАТА`, `БУЛЕВО`, `ЛЮБАЯССЫЛКА`
        // or `Вид.Объект[|Вид.Объект…]`.
        "T" => ParameterValue::Table {
            columns: rest
                .split(',')
                .filter(|column| !column.is_empty())
                .map(|column| {
                    let (name, kind) = column.split_once(':').unwrap_or((column, "ЛЮБАЯССЫЛКА"));
                    ParameterColumn::new(name, column_kind(kind, snapshot))
                })
                .collect(),
            rows: Vec::new(),
        },
        _ => ParameterValue::Null,
    }
}

pub(crate) fn unescape(text: &str) -> String {
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
pub(crate) fn options(
    entry: &CorpusEntry,
    snapshot: &MetadataSnapshot,
) -> (Vec<QueryParameter>, SessionParameters, Vec<QueryParameter>) {
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
                .map_or(ParameterValue::Null, |(_, value)| {
                    parameter_value(value, snapshot)
                });
            QueryParameter::new(name, recorded)
        })
        .collect::<Vec<_>>();
    // `__vt_<Имя>`: a temporary table another batch defines, recorded as
    // the typed columns the text reads; the runner defines it first.
    let predefined = entry
        .parameters
        .iter()
        .filter(|(name, _)| name.starts_with("__vt_"))
        .map(|(name, value)| QueryParameter::new(name, parameter_value(value, snapshot)))
        .collect::<Vec<_>>();
    let mut session = SessionParameters::new();
    super::supply_active_separation_conditions(snapshot, &mut session);
    for (name, value) in [
        (
            "ОбластьДанныхЗначение",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        ("ОбластьДанныхИспользование", ParameterValue::Boolean(true)),
        (
            "ОбластьДанныхОсновныеДанные",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        (
            "ИспользованиеРазделителяСеанса",
            ParameterValue::Boolean(true),
        ),
        (
            "ЗначениеРазделителя",
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ),
        ("ИспользованиеРазделителя", ParameterValue::Boolean(true)),
    ] {
        session.set(QueryParameter::new(name, value));
    }
    (parameters, session, predefined)
}

/// Writes a JSON string the way the corpus files spell one.
pub(crate) fn escape(value: &str) -> String {
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
