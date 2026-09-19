//! Completion for the console: the candidates a prompt offers and
//! how they are matched against what has been typed.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::query::queryable_field_catalog;
use open_sdbl::{TokenKind, tokenize};
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

use super::*;

#[cfg(test)]
#[path = "../tests/repl_completion.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PresentationPlanKey {
    pub(super) object: ObjectId,
    pub(super) language: &'static str,
    pub(super) policy_version: u32,
}

#[derive(Debug, Clone)]
pub(super) struct CompletionName {
    value: String,
    key: String,
    dots: usize,
}

impl CompletionName {
    pub(super) fn new(value: String) -> Self {
        Self {
            key: value.to_lowercase(),
            dots: value.bytes().filter(|byte| *byte == b'.').count(),
            value,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CompletionPath {
    prefixes: Vec<CompletionName>,
    suffixes: Vec<CompletionName>,
}

impl CompletionPath {
    pub(super) fn new(prefixes: Vec<String>, suffixes: Vec<String>) -> Self {
        Self {
            prefixes: prefixes.into_iter().map(CompletionName::new).collect(),
            suffixes: suffixes.into_iter().map(CompletionName::new).collect(),
        }
    }

    #[cfg(test)]
    pub(super) fn stored_names(&self) -> usize {
        self.prefixes.len() + self.suffixes.len()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConsoleHelper {
    candidates: Vec<CompletionName>,
    source_candidates: Vec<CompletionName>,
    paths: Vec<CompletionPath>,
    known_identifiers: HashSet<String>,
    parameters: Vec<CompletionName>,
    temporary_tables: Vec<CompletionName>,
}

impl ConsoleHelper {
    pub(super) fn from_snapshot(snapshot: &MetadataSnapshot) -> Self {
        let mut candidates = [
            "\\dt",
            "\\di",
            "\\d",
            "\\refresh",
            "\\set",
            "\\params",
            "\\unset",
            "\\session",
            "\\restrict",
            "\\users",
            "\\user",
            "\\roles",
            "\\role",
            "\\template",
            "\\rls",
            "\\as",
            "\\tables",
            "\\help",
            "\\q",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        candidates.extend(COMPLETION_KEYWORDS.iter().map(|value| (*value).to_owned()));
        let mut candidate_keys = candidates
            .iter()
            .map(|candidate| candidate.to_lowercase())
            .collect::<HashSet<_>>();
        let mut source_candidates = Vec::new();
        let mut source_candidate_keys = HashSet::new();
        let mut paths = Vec::new();

        let fields_by_object = queryable_field_catalog(snapshot);
        let mut object_by_table = HashMap::new();
        for object in snapshot.objects() {
            let object_id = ObjectId::from(&object.guid);
            if let Some(table) = object.physical_table.as_deref() {
                object_by_table
                    .entry(normalize_physical_table(table))
                    .or_insert(object_id);
            }
        }

        if snapshot
            .objects()
            .iter()
            .any(|object| object.kind == Some(MetadataKind::Constant) && object.live)
        {
            for name in CONSTANTS_TABLE_NAMES {
                push_unique(&mut candidates, &mut candidate_keys, name);
            }
        }
        for object in snapshot.objects() {
            let (Some(kind), Some(name)) = (object.kind, object.name.as_deref()) else {
                continue;
            };
            let object_names = [
                name.to_owned(),
                format!("{}.{name}", kind.as_str()),
                format!("{}.{name}", russian_metadata_kind(kind)),
            ];
            let qualified_object_names = &object_names[1..];
            for object_name in &object_names {
                push_unique(&mut candidates, &mut candidate_keys, object_name);
            }
            push_virtual_table_candidates(
                &mut candidates,
                &mut candidate_keys,
                kind,
                &object_names,
            );
            push_service_table_candidates(
                &mut candidates,
                &mut candidate_keys,
                kind,
                &object_names,
                snapshot.objects().iter().any(|candidate| {
                    candidate.kind == Some(MetadataKind::ChangeRegistration)
                        && candidate.owner == Some(ObjectId::from(&object.guid))
                }),
            );
            for object_name in qualified_object_names {
                push_unique(
                    &mut source_candidates,
                    &mut source_candidate_keys,
                    object_name,
                );
            }
            push_virtual_table_candidates(
                &mut source_candidates,
                &mut source_candidate_keys,
                kind,
                qualified_object_names,
            );
            push_service_table_candidates(
                &mut source_candidates,
                &mut source_candidate_keys,
                kind,
                qualified_object_names,
                snapshot.objects().iter().any(|candidate| {
                    candidate.kind == Some(MetadataKind::ChangeRegistration)
                        && candidate.owner == Some(ObjectId::from(&object.guid))
                }),
            );
            if let Some(table) = object.physical_table.as_deref() {
                push_unique(&mut candidates, &mut candidate_keys, table);
            }

            let Some(fields) = fields_by_object.get(&ObjectId::from(&object.guid)) else {
                continue;
            };
            let mut object_fields = Vec::new();
            let mut object_field_keys = HashSet::new();
            for field in fields {
                push_unique(&mut candidates, &mut candidate_keys, &field.name);
                for alias in &field.aliases {
                    push_unique(&mut candidates, &mut candidate_keys, alias);
                    push_unique(&mut object_fields, &mut object_field_keys, alias);
                }

                let Some(target) = field.reference_target.as_deref() else {
                    continue;
                };
                let Some(target_object) = object_by_table.get(&normalize_physical_table(target))
                else {
                    continue;
                };
                let Some(target_fields) = fields_by_object.get(target_object) else {
                    continue;
                };
                let mut target_aliases = Vec::new();
                let mut target_alias_keys = HashSet::new();
                for target_field in target_fields {
                    for target_alias in &target_field.aliases {
                        push_unique(&mut target_aliases, &mut target_alias_keys, target_alias);
                    }
                }
                paths.push(CompletionPath::new(field.aliases.clone(), target_aliases));
            }
            if !object_fields.is_empty() {
                paths.push(CompletionPath::new(object_names.to_vec(), object_fields));
            }
        }

        candidates.sort_by_cached_key(|value| value.to_lowercase());
        source_candidates.sort_by_cached_key(|value| value.to_lowercase());
        let known_identifiers = candidates
            .iter()
            .flat_map(|candidate| candidate.split('.'))
            .filter(|part| !part.starts_with('\\'))
            .map(str::to_lowercase)
            .chain(paths.iter().flat_map(|path| {
                path.prefixes
                    .iter()
                    .chain(&path.suffixes)
                    .map(|name| name.key.clone())
            }))
            .collect();
        Self {
            candidates: candidates.into_iter().map(CompletionName::new).collect(),
            source_candidates: source_candidates
                .into_iter()
                .map(CompletionName::new)
                .collect(),
            paths,
            known_identifiers,
            parameters: Vec::new(),
            temporary_tables: Vec::new(),
        }
    }

    /// Replaces the stored parameter names offered after `&`.
    pub(super) fn set_parameters(&mut self, names: Vec<String>) {
        self.parameters = names
            .into_iter()
            .map(|name| CompletionName::new(format!("&{name}")))
            .collect();
    }

    /// Temporary tables are sources without a metadata qualifier, so they
    /// are offered separately from the dotted metadata names.
    pub(super) fn set_temporary_tables(&mut self, names: Vec<String>) {
        self.temporary_tables = names.into_iter().map(CompletionName::new).collect();
    }

    #[cfg(test)]
    pub(super) fn for_test(
        candidates: Vec<String>,
        source_candidates: Vec<String>,
        known_identifiers: HashSet<String>,
    ) -> Self {
        Self {
            candidates: candidates.into_iter().map(CompletionName::new).collect(),
            source_candidates: source_candidates
                .into_iter()
                .map(CompletionName::new)
                .collect(),
            paths: Vec::new(),
            known_identifiers,
            parameters: Vec::new(),
            temporary_tables: Vec::new(),
        }
    }

    pub(super) fn complete_values(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let start = completion_start(line, pos);
        let prefix = line[start..pos].to_lowercase();
        if prefix.starts_with('&') {
            let mut values = self
                .parameters
                .iter()
                .filter(|candidate| candidate.key.starts_with(&prefix))
                .map(|candidate| Pair {
                    display: candidate.value.clone(),
                    replacement: candidate.value.clone(),
                })
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.display.cmp(&right.display));
            return (start, values);
        }
        let source_context = is_source_completion_context(line, start);
        let candidates = if source_context {
            &self.source_candidates
        } else {
            &self.candidates
        };
        let mut temporary = if source_context {
            self.temporary_tables
                .iter()
                .filter(|candidate| candidate.key.starts_with(&prefix))
                .map(|candidate| {
                    (
                        candidate.key.clone(),
                        Pair {
                            display: candidate.value.clone(),
                            replacement: candidate.value.clone(),
                        },
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let complete_virtual_source = prefix.bytes().filter(|byte| *byte == b'.').count() >= 2;
        let mut values = candidates
            .iter()
            .filter(|candidate| !source_context || complete_virtual_source || candidate.dots == 1)
            .filter(|candidate| candidate.key.starts_with(&prefix))
            .map(|candidate| {
                (
                    candidate.key.clone(),
                    Pair {
                        display: candidate.value.clone(),
                        replacement: candidate.value.clone(),
                    },
                )
            })
            .collect::<Vec<_>>();
        if !source_context && let Some((typed_prefix, typed_suffix)) = prefix.rsplit_once('.') {
            let mut emitted = values
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<HashSet<_>>();
            for path in &self.paths {
                for path_prefix in path
                    .prefixes
                    .iter()
                    .filter(|candidate| candidate.key == typed_prefix)
                {
                    for suffix in path
                        .suffixes
                        .iter()
                        .filter(|candidate| candidate.key.starts_with(typed_suffix))
                    {
                        let key = format!("{}.{}", path_prefix.key, suffix.key);
                        if emitted.insert(key.clone()) {
                            let value = format!("{}.{}", path_prefix.value, suffix.value);
                            values.push((
                                key,
                                Pair {
                                    display: value.clone(),
                                    replacement: value,
                                },
                            ));
                        }
                    }
                }
            }
        }
        values.append(&mut temporary);
        values.sort_by(|left, right| left.0.cmp(&right.0));
        values.dedup_by(|left, right| left.0 == right.0);
        (start, values.into_iter().map(|(_, pair)| pair).collect())
    }
}

impl Completer for ConsoleHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _context: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        Ok(self.complete_values(line, pos))
    }
}

impl Hinter for ConsoleHelper {
    type Hint = String;
}

impl Validator for ConsoleHelper {}
impl Helper for ConsoleHelper {}

impl Highlighter for ConsoleHelper {
    fn highlight<'line>(&self, line: &'line str, _pos: usize) -> Cow<'line, str> {
        if line.trim_start().starts_with('\\') {
            return Cow::Owned(format!("\x1b[1;36m{line}\x1b[0m"));
        }
        let Ok(tokens) = tokenize(line) else {
            return Cow::Borrowed(line);
        };
        let mut rendered = String::with_capacity(line.len() + tokens.len() * 9);
        let mut end = 0;
        let mut styled = false;
        for token in tokens {
            rendered.push_str(&line[end..token.span.start]);
            let style = match token.kind {
                TokenKind::Keyword(_) => Some("\x1b[1;34m"),
                TokenKind::String => Some("\x1b[32m"),
                TokenKind::Number => Some("\x1b[33m"),
                TokenKind::Parameter => Some("\x1b[35m"),
                TokenKind::Comment => Some("\x1b[2;37m"),
                TokenKind::Identifier
                    if self
                        .known_identifiers
                        .contains(&token.lexeme.to_lowercase()) =>
                {
                    Some("\x1b[36m")
                }
                _ => None,
            };
            if let Some(style) = style {
                styled = true;
                rendered.push_str(style);
                rendered.push_str(token.lexeme);
                rendered.push_str("\x1b[0m");
            } else {
                rendered.push_str(token.lexeme);
            }
            end = token.span.end;
        }
        rendered.push_str(&line[end..]);
        if styled {
            Cow::Owned(rendered)
        } else {
            Cow::Borrowed(line)
        }
    }

    fn highlight_prompt<'buffer, 'self_lifetime: 'buffer, 'prompt: 'buffer>(
        &'self_lifetime self,
        prompt: &'prompt str,
        _default: bool,
    ) -> Cow<'buffer, str> {
        Cow::Owned(format!("\x1b[1;32m{prompt}\x1b[0m"))
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _kind: rustyline::highlight::CmdKind,
    ) -> bool {
        true
    }
}

pub(super) fn completion_start(line: &str, pos: usize) -> usize {
    line[..pos]
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!is_completion_character(character)).then_some(index + character.len_utf8())
        })
        .unwrap_or(0)
}

pub(super) fn is_completion_character(character: char) -> bool {
    matches!(character, '\\' | '_' | '.' | '&') || character.is_alphanumeric()
}

pub(super) fn is_source_completion_context(line: &str, start: usize) -> bool {
    line[..start]
        .split_whitespace()
        .next_back()
        .map(str::to_lowercase)
        .is_some_and(|keyword| matches!(keyword.as_str(), "из" | "from" | "соединение" | "join"))
}

pub(super) fn push_unique(values: &mut Vec<String>, keys: &mut HashSet<String>, value: &str) {
    if !value.is_empty() && keys.insert(value.to_lowercase()) {
        values.push(value.to_owned());
    }
}

pub(super) fn push_virtual_table_candidates(
    candidates: &mut Vec<String>,
    candidate_keys: &mut HashSet<String>,
    kind: MetadataKind,
    object_names: &[String],
) {
    let suffixes: &[&str] = match kind {
        MetadataKind::InformationRegister => &[
            "СрезПоследних()",
            "SliceLast()",
            "СрезПервых()",
            "SliceFirst()",
        ],
        MetadataKind::AccumulationRegister => {
            &["Остатки()", "Balance()", "Обороты()", "Turnovers()"]
        }
        _ => &[],
    };
    for object_name in object_names {
        for suffix in suffixes {
            push_unique(
                candidates,
                candidate_keys,
                &format!("{object_name}.{suffix}"),
            );
        }
    }
}

pub(super) fn push_service_table_candidates(
    candidates: &mut Vec<String>,
    candidate_keys: &mut HashSet<String>,
    kind: MetadataKind,
    object_names: &[String],
    has_change_registration: bool,
) {
    let suffixes: &[&str] = match kind {
        MetadataKind::ChartOfCalculationTypes => &[
            "БазовыеВидыРасчета",
            "BaseCalculationKinds",
            "ВедущиеВидыРасчета",
            "LeadingCalculationKinds",
            "ВытесняющиеВидыРасчета",
            "DisplacedCalculationKinds",
        ],
        MetadataKind::ChartOfAccounts => &["ВидыСубконто", "ExtraDimensions"],
        _ => &[],
    };
    for object_name in object_names {
        if has_change_registration {
            for suffix in ["Изменения", "Changes"] {
                push_unique(
                    candidates,
                    candidate_keys,
                    &format!("{object_name}.{suffix}"),
                );
            }
        }
        for suffix in suffixes {
            push_unique(
                candidates,
                candidate_keys,
                &format!("{object_name}.{suffix}"),
            );
        }
    }
}

pub(super) fn normalize_physical_table(table: &str) -> String {
    table.strip_prefix('_').unwrap_or(table).to_lowercase()
}

pub(super) const fn russian_metadata_kind(kind: MetadataKind) -> &'static str {
    match kind {
        MetadataKind::Catalog => "Справочник",
        MetadataKind::Document => "Документ",
        MetadataKind::Enumeration => "Перечисление",
        MetadataKind::InformationRegister => "РегистрСведений",
        MetadataKind::AccumulationRegister => "РегистрНакопления",
        MetadataKind::AccountingRegister => "РегистрБухгалтерии",
        MetadataKind::CalculationRegister => "РегистрРасчета",
        MetadataKind::ChartOfCharacteristicTypes => "ПланВидовХарактеристик",
        MetadataKind::ChartOfCalculationTypes => "ПланВидовРасчета",
        MetadataKind::ChartOfAccounts => "ПланСчетов",
        MetadataKind::Constant => "Константа",
        MetadataKind::ExchangePlan => "ПланОбмена",
        MetadataKind::BusinessProcess => "БизнесПроцесс",
        MetadataKind::Task => "Задача",
        MetadataKind::Sequence => "Последовательность",
        MetadataKind::ChangeRegistration => "РегистрацияИзменений",
        MetadataKind::Recalculation => "Перерасчет",
        MetadataKind::CalculationKindDependency => "ЗависимостьВидовРасчета",
        MetadataKind::ExtraDimension => "ВидСубконто",
        MetadataKind::ResolveOnlyService => "СлужебнаяТаблица",
        _ => "Метаданные",
    }
}
