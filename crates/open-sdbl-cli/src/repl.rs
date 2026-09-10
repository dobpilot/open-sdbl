use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, IsTerminal, Write};
#[cfg(target_os = "linux")]
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

use open_sdbl::metadata::{MetadataKind, MetadataObject, MetadataSnapshot, ObjectId};
use open_sdbl::query::{
    ColumnKind, CompileOptions, CompiledQuery, MsSqlBackend, PostgresBackend, Prepared,
    PresentationExpression, PresentationPlan, PresentationRequest, QueryCompiler, QueryParameter,
    TempTablesManager, find_metadata_object, queryable_field_catalog, queryable_fields,
};
use open_sdbl::{TokenKind, tokenize};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};
use unicode_width::UnicodeWidthStr;

use super::cells::Cell;
use super::params::{ParameterStore, apply_parameter_command, parse_parameter_command};
use super::{
    CliError, DatabaseDialect, DatabaseSession, MAX_CELL_WIDTH, MAX_PRINTED_ROWS, QueryRows,
    bounded_field, escape_field, yes_no,
};

const CONSOLE_HELP: &str = "Commands:
  \\dt                 list resolved metadata tables
  \\di                 list declared and live indexes
  \\d <metadata-name>  describe attributes and indexes
  \\refresh            reload DBNames, Config, SchemaStorage, and catalogs
  \\set <name> <lit>   store a query parameter (&name) from an SDBL literal
  \\params             list stored parameters
  \\unset <name>       remove a stored parameter
  \\tables             list temporary tables placed in this session
  \\help               show this help
  \\q                  quit

Enter a supported 1C SELECT query and terminate it with a semicolon.
Statements placing temporary tables (ПОМЕСТИТЬ, ДОБАВИТЬ, УНИЧТОЖИТЬ) keep
them for the rest of the session; \\refresh forgets them.
";

#[cfg(any(target_os = "linux", test))]
const COMMAND_HINT: &str =
    "\\dt tables  \\di indexes  \\d <name> describe  \\refresh reload  \\help  \\q quit";

type ConsoleEditor = Editor<ConsoleHelper, DefaultHistory>;

const COMPLETION_KEYWORDS: &[&str] = &[
    "ВЫБРАТЬ",
    "SELECT",
    "ИЗ",
    "FROM",
    "ГДЕ",
    "WHERE",
    "КАК",
    "AS",
    "И",
    "AND",
    "ИЛИ",
    "OR",
    "НЕ",
    "NOT",
    "В",
    "IN",
    "ЕСТЬ",
    "IS",
    "NULL",
    "ИСТИНА",
    "TRUE",
    "ЛОЖЬ",
    "FALSE",
    "РАЗЛИЧНЫЕ",
    "DISTINCT",
    "ПЕРВЫЕ",
    "TOP",
    "УПОРЯДОЧИТЬ",
    "ORDER",
    "ПО",
    "BY",
    "СГРУППИРОВАТЬ",
    "GROUP",
    "ИМЕЮЩИЕ",
    "HAVING",
    "ОБЪЕДИНИТЬ",
    "UNION",
    "ВСЕ",
    "ALL",
    "ПОМЕСТИТЬ",
    "INTO",
    "СОЕДИНЕНИЕ",
    "JOIN",
    "ЛЕВОЕ",
    "LEFT",
    "ПРАВОЕ",
    "RIGHT",
    "ПОЛНОЕ",
    "FULL",
    "ВНУТРЕННЕЕ",
    "INNER",
    "ВНЕШНЕЕ",
    "OUTER",
    "ON",
    "ВЫБОР",
    "CASE",
    "КОГДА",
    "WHEN",
    "ТОГДА",
    "THEN",
    "ИНАЧЕ",
    "ELSE",
    "КОНЕЦ",
    "END",
    "ПРЕДСТАВЛЕНИЕССЫЛКИ",
    "REFPRESENTATION",
    "ПРЕДСТАВЛЕНИЕ",
    "PRESENTATION",
    "КОЛИЧЕСТВО",
    "COUNT",
    "СУММА",
    "SUM",
    "МИНИМУМ",
    "MIN",
    "МАКСИМУМ",
    "MAX",
    "СРЕЗПОСЛЕДНИХ",
    "SLICELAST",
    "СРЕЗПЕРВЫХ",
    "SLICEFIRST",
    "ОСТАТКИ",
    "BALANCE",
    "ОБОРОТЫ",
    "TURNOVERS",
    "ДАТАВРЕМЯ",
    "DATETIME",
    "НАЧАЛОПЕРИОДА",
    "BEGINOFPERIOD",
    "ЗНАЧЕНИЕ",
    "VALUE",
    "УНИКАЛЬНЫЙИДЕНТИФИКАТОР",
    "UUID",
    "ВЫРАЗИТЬ",
    "CAST",
    "ЕСТЬNULL",
    "ISNULL",
    "ПОДОБНО",
    "LIKE",
    "СПЕЦСИМВОЛ",
    "ESCAPE",
];

const PRESENTATION_POLICY_VERSION: u32 = 2;
const MAX_INPUT_LINE_BYTES: usize = 1024 * 1024;
const UNRESOLVED_REFERENCE: &str = "<unresolved reference>";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PresentationPlanKey {
    object: ObjectId,
    language: &'static str,
    policy_version: u32,
}

#[derive(Debug, Clone)]
struct CompletionName {
    value: String,
    key: String,
    dots: usize,
}

impl CompletionName {
    fn new(value: String) -> Self {
        Self {
            key: value.to_lowercase(),
            dots: value.bytes().filter(|byte| *byte == b'.').count(),
            value,
        }
    }
}

#[derive(Debug, Clone)]
struct CompletionPath {
    prefixes: Vec<CompletionName>,
    suffixes: Vec<CompletionName>,
}

impl CompletionPath {
    fn new(prefixes: Vec<String>, suffixes: Vec<String>) -> Self {
        Self {
            prefixes: prefixes.into_iter().map(CompletionName::new).collect(),
            suffixes: suffixes.into_iter().map(CompletionName::new).collect(),
        }
    }

    #[cfg(test)]
    fn stored_names(&self) -> usize {
        self.prefixes.len() + self.suffixes.len()
    }
}

#[derive(Debug, Clone)]
struct ConsoleHelper {
    candidates: Vec<CompletionName>,
    source_candidates: Vec<CompletionName>,
    paths: Vec<CompletionPath>,
    known_identifiers: HashSet<String>,
    parameters: Vec<CompletionName>,
    temporary_tables: Vec<CompletionName>,
}

impl ConsoleHelper {
    fn from_snapshot(snapshot: &MetadataSnapshot) -> Self {
        let mut candidates = [
            "\\dt",
            "\\di",
            "\\d",
            "\\refresh",
            "\\set",
            "\\params",
            "\\unset",
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
    fn set_parameters(&mut self, names: Vec<String>) {
        self.parameters = names
            .into_iter()
            .map(|name| CompletionName::new(format!("&{name}")))
            .collect();
    }

    /// Temporary tables are sources without a metadata qualifier, so they
    /// are offered separately from the dotted metadata names.
    fn set_temporary_tables(&mut self, names: Vec<String>) {
        self.temporary_tables = names.into_iter().map(CompletionName::new).collect();
    }

    #[cfg(test)]
    fn for_test(
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

    fn complete_values(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
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

fn completion_start(line: &str, pos: usize) -> usize {
    line[..pos]
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!is_completion_character(character)).then_some(index + character.len_utf8())
        })
        .unwrap_or(0)
}

fn is_completion_character(character: char) -> bool {
    matches!(character, '\\' | '_' | '.' | '&') || character.is_alphanumeric()
}

fn is_source_completion_context(line: &str, start: usize) -> bool {
    line[..start]
        .split_whitespace()
        .next_back()
        .map(str::to_lowercase)
        .is_some_and(|keyword| matches!(keyword.as_str(), "из" | "from" | "соединение" | "join"))
}

fn push_unique(values: &mut Vec<String>, keys: &mut HashSet<String>, value: &str) {
    if !value.is_empty() && keys.insert(value.to_lowercase()) {
        values.push(value.to_owned());
    }
}

fn push_virtual_table_candidates(
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

fn push_service_table_candidates(
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

fn normalize_physical_table(table: &str) -> String {
    table.strip_prefix('_').unwrap_or(table).to_lowercase()
}

const fn russian_metadata_kind(kind: MetadataKind) -> &'static str {
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

enum PreparedQuery {
    Postgres(Prepared<PostgresBackend>),
    MsSql(Prepared<MsSqlBackend>),
}

impl PreparedQuery {
    fn presentation_request(&self) -> &PresentationRequest {
        match self {
            Self::Postgres(query) => query.presentation_request(),
            Self::MsSql(query) => query.presentation_request(),
        }
    }

    /// Compiles the prepared batch, updating the session's temporary tables.
    /// `None` means the batch only dropped tables and has nothing to run.
    fn compile_batch(
        self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
        parameters: &[QueryParameter],
        temporary: &mut TempTablesManager,
    ) -> Result<Option<CompiledQuery>, open_sdbl::query::QueryDiagnostic> {
        let options = CompileOptions::new()
            .presentations(plans)
            .parameters(parameters);
        match self {
            Self::Postgres(query) => query.compile_batch(snapshot, &options, temporary),
            Self::MsSql(query) => query.compile_batch(snapshot, &options, temporary),
        }
    }
}

pub(super) async fn run(
    session: &mut DatabaseSession,
    mut snapshot: MetadataSnapshot,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let interactive = io::stdin().is_terminal();
    let _terminal_guard = TerminalUtf8Guard::enable(interactive)?;
    let mut editor = if interactive {
        let mut editor = ConsoleEditor::new()
            .map_err(|error| terminal_error("cannot initialize console line editor", error))?;
        editor.set_helper(Some(ConsoleHelper::from_snapshot(&snapshot)));
        Some(editor)
    } else {
        None
    };
    let mut input = BufReader::new(tokio::io::stdin());
    let mut line = Vec::new();
    let mut statement = String::new();
    let mut presentation_cache = HashMap::new();
    let mut parameters = ParameterStore::new();
    let mut temporary_tables = TempTablesManager::new();

    if interactive {
        writeln!(output, "open-sdbl 1C query console. Type \\help for help.")
            .map_err(CliError::standard_output)?;
        if let Some(description) = session.server_description() {
            writeln!(output, "{description}").map_err(CliError::standard_output)?;
        }
    }
    let mut footer = PinnedFooter::enable(interactive)?;
    loop {
        output.flush().map_err(CliError::standard_output)?;
        footer.redraw()?;

        line.clear();
        let bytes = if let Some(editor) = editor.as_mut() {
            let prompt = if statement.is_empty() {
                "open-sdbl=> "
            } else {
                "       ...> "
            };
            match tokio::task::block_in_place(|| editor.readline(prompt)) {
                Ok(value) => {
                    line.extend_from_slice(value.as_bytes());
                    line.push(b'\n');
                    line.len()
                }
                Err(ReadlineError::Interrupted) => {
                    writeln!(output, "^C").map_err(CliError::standard_output)?;
                    statement.clear();
                    continue;
                }
                Err(ReadlineError::Eof) => 0,
                Err(error) => {
                    return Err(terminal_error("cannot read console input", error));
                }
            }
        } else {
            match read_bounded_line(&mut input, &mut line, MAX_INPUT_LINE_BYTES)
                .await
                .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?
            {
                BoundedLine::Read(bytes) => bytes,
                BoundedLine::TooLong => {
                    eprintln!(
                        "error: input line exceeds {MAX_INPUT_LINE_BYTES} bytes; current statement discarded"
                    );
                    statement.clear();
                    continue;
                }
            }
        };
        if bytes == 0 {
            if !statement.trim().is_empty() {
                eprintln!("error: incomplete query at end of input; expected ';'");
            }
            return Ok(());
        }

        let line = match decode_input_line(&line) {
            Ok(line) => line,
            Err(offset) => {
                eprintln!(
                    "error: input is not valid UTF-8 at byte {}; current statement discarded",
                    offset + 1
                );
                statement.clear();
                continue;
            }
        };

        if statement.is_empty() && line.trim_start().starts_with('\\') {
            add_history(&mut editor, line.trim());
            if let Some(command) = parse_parameter_command(line.trim()) {
                match apply_parameter_command(&mut parameters, command, &snapshot) {
                    Ok(text) => {
                        output
                            .write_all(text.as_bytes())
                            .map_err(CliError::standard_output)?;
                        if let Some(helper) = editor.as_mut().and_then(Editor::helper_mut) {
                            helper.set_parameters(parameters.names());
                        }
                    }
                    Err(error) => eprintln!("error: {}", escape_field(&error.to_string())),
                }
                continue;
            }
            match execute_meta_command(
                session,
                &mut snapshot,
                &temporary_tables,
                line.trim(),
                output,
            )
            .await
            {
                Ok(MetaOutcome::Continue) => {}
                Ok(MetaOutcome::Refreshed) => {
                    presentation_cache.clear();
                    // Definitions hold SQL generated against the old
                    // snapshot, so they cannot survive a reload.
                    if !temporary_tables.is_empty() {
                        temporary_tables.clear();
                        writeln!(output, "Temporary tables cleared.")
                            .map_err(CliError::standard_output)?;
                    }
                    if let Some(helper) = editor.as_mut().and_then(Editor::helper_mut) {
                        *helper = ConsoleHelper::from_snapshot(&snapshot);
                        helper.set_parameters(parameters.names());
                    }
                }
                Ok(MetaOutcome::Quit) => return Ok(()),
                Err(error) => {
                    eprintln!("error: {}", escape_field(&error.to_string()));
                    ensure_session_remains_usable(session.is_dead())?;
                }
            }
            continue;
        }

        statement.push_str(line);
        if !statement_is_complete(&statement) {
            continue;
        }

        add_history(&mut editor, statement.trim());
        let generation_started = Instant::now();
        let prepared = match session.dialect() {
            DatabaseDialect::Postgres => QueryCompiler::new(&snapshot, PostgresBackend)
                .prepare_with(&statement, &temporary_tables)
                .map(PreparedQuery::Postgres),
            DatabaseDialect::MsSql { backend } => QueryCompiler::new(&snapshot, backend)
                .prepare_with(&statement, &temporary_tables)
                .map(PreparedQuery::MsSql),
        };
        let placed_before = temporary_table_names(&temporary_tables);
        let compilation = match prepared {
            Ok(prepared) => {
                let plans = presentation_plans(
                    &mut presentation_cache,
                    &snapshot,
                    prepared.presentation_request(),
                );
                let values = parameters.values_for(&statement);
                prepared.compile_batch(&snapshot, &plans, &values, &mut temporary_tables)
            }
            Err(error) => Err(error),
        };
        let generation_elapsed = generation_started.elapsed();
        if compilation.is_ok()
            && let Some(helper) = editor.as_mut().and_then(Editor::helper_mut)
        {
            helper.set_temporary_tables(temporary_table_names(&temporary_tables));
        }
        match compilation {
            Ok(None) => {
                let dropped = placed_before
                    .into_iter()
                    .filter(|name| !temporary_tables.contains(name))
                    .collect::<Vec<_>>();
                writeln!(
                    output,
                    "{}",
                    timing_line("SQL generation", generation_elapsed)
                )
                .map_err(CliError::standard_output)?;
                if dropped.is_empty() {
                    writeln!(output, "No statement to execute.")
                } else {
                    writeln!(
                        output,
                        "Temporary tables dropped: {}.",
                        escape_field(&dropped.join(", "))
                    )
                }
                .map_err(CliError::standard_output)?;
                statement.clear();
                continue;
            }
            Ok(Some(compiled)) => {
                writeln!(
                    output,
                    "{}",
                    timing_line("SQL generation", generation_elapsed)
                )
                .and_then(|()| writeln!(output, "SQL: {}", escape_field(&compiled.sql)))
                .map_err(CliError::standard_output)?;
                output.flush().map_err(CliError::standard_output)?;
                let execution_started = Instant::now();
                let execution = if interactive {
                    let cancellation = session.cancellation();
                    let execution = tokio::select! {
                        result = session.query(&compiled.sql, compiled.columns.len()) => Some(result),
                        signal = tokio::signal::ctrl_c() => {
                            signal.map_err(|error| {
                                CliError::Io("cannot listen for Ctrl-C".to_owned(), error)
                            })?;
                            None
                        }
                    };
                    if execution.is_none() {
                        footer.restore()?;
                        writeln!(output, "^C cancelling query")
                            .map_err(CliError::standard_output)?;
                        output.flush().map_err(CliError::standard_output)?;
                        tokio::select! {
                            result = session.cancel_query(cancellation) => result?,
                            signal = tokio::signal::ctrl_c() => {
                                signal.map_err(|error| {
                                    CliError::Io("cannot listen for Ctrl-C".to_owned(), error)
                                })?;
                                return Err(CliError::Terminal(
                                    "query cancellation interrupted by a second Ctrl-C".to_owned(),
                                ));
                            }
                        }
                        writeln!(output, "query cancelled").map_err(CliError::standard_output)?;
                    }
                    execution
                } else {
                    Some(session.query(&compiled.sql, compiled.columns.len()).await)
                };
                let Some(execution) = execution else {
                    statement.clear();
                    continue;
                };
                match execution {
                    Ok(mut rows) => {
                        let resolution = resolve_deferred_presentations(
                            session,
                            &snapshot,
                            &mut presentation_cache,
                            &compiled,
                            &mut rows,
                        )
                        .await;
                        let execution_elapsed = execution_started.elapsed();
                        if let Err(error) = resolution {
                            let error = escape_field(&error.to_string());
                            eprintln!(
                                "error: {error} ({}: {})",
                                session.execution_label(),
                                format_duration(execution_elapsed)
                            );
                            ensure_session_remains_usable(session.is_dead())?;
                            statement.clear();
                            continue;
                        }
                        writeln!(
                            output,
                            "{}",
                            timing_line(session.execution_label(), execution_elapsed)
                        )
                        .map_err(CliError::standard_output)?;
                        validate_query_rows(&compiled, &rows)?;
                        print_query_rows(output, &compiled, &rows)
                            .map_err(CliError::standard_output)?;
                    }
                    Err(error) => {
                        let execution_elapsed = execution_started.elapsed();
                        let error = escape_field(&error.to_string());
                        eprintln!(
                            "error: {error} ({}: {})",
                            session.execution_label(),
                            format_duration(execution_elapsed)
                        );
                        ensure_session_remains_usable(session.is_dead())?;
                    }
                }
            }
            Err(error) => {
                let error = escape_field(&error.to_string());
                eprintln!(
                    "error: {error} (SQL generation: {})",
                    format_duration(generation_elapsed)
                );
            }
        }
        statement.clear();
    }
}

fn ensure_session_remains_usable(dead: bool) -> Result<(), CliError> {
    if dead {
        Err(CliError::Database(
            "database session is no longer usable; reconnect required".to_owned(),
        ))
    } else {
        Ok(())
    }
}

fn presentation_plans(
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    snapshot: &MetadataSnapshot,
    request: &PresentationRequest,
) -> Vec<PresentationPlan> {
    let mut plans = Vec::with_capacity(request.targets.len());
    for target in &request.targets {
        plans.push(presentation_plan(cache, snapshot, target.object));
    }
    plans
}

fn presentation_plan(
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    snapshot: &MetadataSnapshot,
    object: ObjectId,
) -> PresentationPlan {
    let key = PresentationPlanKey {
        object,
        language: "ru",
        policy_version: PRESENTATION_POLICY_VERSION,
    };
    cache
        .entry(key)
        .or_insert_with(|| default_presentation_plan(snapshot, object))
        .clone()
}

async fn resolve_deferred_presentations(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    compiled: &CompiledQuery,
    rows: &mut QueryRows,
) -> Result<(), CliError> {
    if compiled.deferred_presentations.is_empty() || rows.is_empty() {
        return Ok(());
    }

    let mut cells = Vec::new();
    let mut references = BTreeMap::<ObjectId, BTreeSet<[u8; 16]>>::new();
    for (row_index, row) in rows.iter_mut().enumerate() {
        for &column_index in &compiled.deferred_presentations {
            let cell = row.get_mut(column_index).ok_or_else(|| {
                CliError::Data(format!(
                    "database row has no deferred presentation column {column_index}"
                ))
            })?;
            if cell.is_null() {
                *cell = Cell::Text(UNRESOLVED_REFERENCE.to_owned());
                continue;
            }
            let payload = cell.as_bytes().ok_or_else(|| {
                CliError::Data(
                    "database returned a non-binary deferred presentation payload".to_owned(),
                )
            })?;
            let Some((object, reference)) = decode_deferred_reference(payload, snapshot)? else {
                *cell = Cell::Text(String::new());
                continue;
            };
            references.entry(object).or_default().insert(reference);
            cells.push((row_index, column_index, object, reference));
        }
    }

    let dialect = session.dialect();
    let mut presentations = HashMap::<(ObjectId, [u8; 16]), String>::new();
    for (object, object_references) in references {
        let plan = presentation_plan(cache, snapshot, object);
        let object_references = object_references.into_iter().collect::<Vec<_>>();
        for chunk in object_references.chunks(512) {
            let lookup = match dialect {
                DatabaseDialect::Postgres => QueryCompiler::new(snapshot, PostgresBackend)
                    .compile_presentation_lookup(&plan, chunk),
                DatabaseDialect::MsSql { backend } => {
                    QueryCompiler::new(snapshot, backend).compile_presentation_lookup(&plan, chunk)
                }
            }
            .map_err(|error| {
                CliError::Data(format!(
                    "cannot compile deferred presentation lookup: {error}"
                ))
            })?;
            let lookup_rows = session.query(&lookup.sql, lookup.columns.len()).await?;
            for row in lookup_rows {
                let key = row.first().and_then(Cell::as_bytes).ok_or_else(|| {
                    CliError::Data(
                        "presentation lookup returned a row without a reference".to_owned(),
                    )
                })?;
                let reference = <[u8; 16]>::try_from(key).map_err(|_| {
                    CliError::Data(format!(
                        "presentation lookup reference has {} bytes, expected 16",
                        key.len()
                    ))
                })?;
                let presentation = row
                    .get(1)
                    .and_then(Cell::as_text)
                    .map(str::to_owned)
                    .unwrap_or_default();
                presentations.insert((object, reference), presentation);
            }
        }
    }

    for (row_index, column_index, object, reference) in cells {
        let presentation = resolved_presentation(&presentations, object, reference);
        rows[row_index][column_index] = Cell::Text(presentation);
    }
    Ok(())
}

fn resolved_presentation(
    presentations: &HashMap<(ObjectId, [u8; 16]), String>,
    object: ObjectId,
    reference: [u8; 16],
) -> String {
    presentations
        .get(&(object, reference))
        .cloned()
        .unwrap_or_else(|| UNRESOLVED_REFERENCE.to_owned())
}

/// Splits the 20-byte `RTRef ‖ RRRef` payload into the big-endian table
/// number and the reference; an all-zero reference is the empty 1C reference.
fn split_deferred_payload(payload: &[u8]) -> Result<Option<(u32, [u8; 16])>, CliError> {
    let (database_type, reference) = match (
        payload
            .get(..4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok()),
        payload
            .get(4..)
            .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok()),
    ) {
        (Some(database_type), Some(reference)) => (database_type, reference),
        _ => {
            return Err(CliError::Data(format!(
                "deferred presentation payload has {} bytes, expected 20",
                payload.len()
            )));
        }
    };
    if reference.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    Ok(Some((u32::from_be_bytes(database_type), reference)))
}

fn decode_deferred_reference(
    payload: &[u8],
    snapshot: &MetadataSnapshot,
) -> Result<Option<(ObjectId, [u8; 16])>, CliError> {
    let Some((database_type, reference)) = split_deferred_payload(payload)? else {
        return Ok(None);
    };
    let object = snapshot
        .object_id_by_database_type(database_type)
        .map_err(|error| {
            CliError::Data(format!(
                "runtime reference type {database_type} is absent from metadata: {error}"
            ))
        })?;
    Ok(Some((object, reference)))
}

fn default_presentation_plan(snapshot: &MetadataSnapshot, object: ObjectId) -> PresentationPlan {
    let metadata_object = snapshot.object_by_id(object);
    let kind = metadata_object.and_then(|object| object.kind);
    let type_name = metadata_object.map_or_else(
        || "Документ".to_owned(),
        |metadata_object| {
            snapshot
                .descriptors()
                .iter()
                .find(|descriptor| descriptor.object_guid == metadata_object.guid)
                .and_then(|descriptor| {
                    descriptor
                        .synonyms
                        .iter()
                        .find(|synonym| synonym.language.eq_ignore_ascii_case("ru"))
                        .map(|synonym| synonym.text.trim())
                        .filter(|text| !text.is_empty())
                })
                .map(str::to_owned)
                .or_else(|| metadata_object.name.clone())
                .unwrap_or_else(|| "Документ".to_owned())
        },
    );
    let description = snapshot.field_id(object, "Наименование").ok();
    let code = snapshot.field_id(object, "Код").ok();
    let number = snapshot.field_id(object, "Номер").ok();
    let date = snapshot.field_id(object, "Дата").ok();
    let id = snapshot.field_id(object, "Ссылка").ok();

    let (fields, expression) =
        default_presentation_template(kind, &type_name, description, code, number, date, id);
    PresentationPlan {
        object,
        fields,
        expression,
    }
}

#[allow(clippy::too_many_arguments)]
fn default_presentation_template(
    kind: Option<MetadataKind>,
    type_name: &str,
    description: Option<open_sdbl::metadata::FieldId>,
    code: Option<open_sdbl::metadata::FieldId>,
    number: Option<open_sdbl::metadata::FieldId>,
    date: Option<open_sdbl::metadata::FieldId>,
    id: Option<open_sdbl::metadata::FieldId>,
) -> (Vec<open_sdbl::metadata::FieldId>, PresentationExpression) {
    if kind == Some(MetadataKind::Catalog)
        && let (Some(description), Some(code)) = (description, code)
    {
        return (
            vec![description, code],
            PresentationExpression::Concat(vec![
                PresentationExpression::Field(description),
                PresentationExpression::Literal(" (".to_owned()),
                PresentationExpression::Field(code),
                PresentationExpression::Literal(")".to_owned()),
            ]),
        );
    }
    if kind == Some(MetadataKind::Document) {
        return match (number, date) {
            (Some(number), Some(date)) => (
                vec![number, date],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" ".to_owned()),
                    PresentationExpression::Field(number),
                    PresentationExpression::Literal(" от ".to_owned()),
                    PresentationExpression::Field(date),
                ]),
            ),
            (Some(number), None) => (
                vec![number],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" ".to_owned()),
                    PresentationExpression::Field(number),
                ]),
            ),
            (None, Some(date)) => (
                vec![date],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" от ".to_owned()),
                    PresentationExpression::Field(date),
                ]),
            ),
            (None, None) => (
                Vec::new(),
                PresentationExpression::Literal(type_name.to_owned()),
            ),
        };
    }

    match (description, code) {
        (Some(description), Some(code)) => (
            vec![description, code],
            PresentationExpression::Concat(vec![
                PresentationExpression::Field(description),
                PresentationExpression::Literal(" (".to_owned()),
                PresentationExpression::Field(code),
                PresentationExpression::Literal(")".to_owned()),
            ]),
        ),
        (Some(field), None) | (None, Some(field)) => {
            (vec![field], PresentationExpression::Field(field))
        }
        (None, None) => match number.or(id) {
            Some(field) => (vec![field], PresentationExpression::Field(field)),
            None => (Vec::new(), PresentationExpression::Literal(String::new())),
        },
    }
}

enum MetaOutcome {
    Continue,
    Refreshed,
    Quit,
}

async fn execute_meta_command(
    session: &mut DatabaseSession,
    snapshot: &mut MetadataSnapshot,
    temporary: &TempTablesManager,
    command: &str,
    output: &mut impl Write,
) -> Result<MetaOutcome, CliError> {
    match command {
        "\\q" => Ok(MetaOutcome::Quit),
        "\\help" | "\\?" => {
            output
                .write_all(CONSOLE_HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\dt" => {
            print_tables(output, snapshot).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\di" => {
            print_indexes(output, snapshot).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\tables" => {
            print_temporary_tables(output, temporary).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\refresh" => {
            *snapshot = session.metadata().await?;
            writeln!(output, "Metadata refreshed.").map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Refreshed)
        }
        _ if command == "\\d" => Err(CliError::Data(
            "usage: \\d <qualified-or-unique-metadata-name>".to_owned(),
        )),
        _ if command.starts_with("\\d ") || command.starts_with("\\d\t") => {
            let name = command[2..].trim();
            print_description(output, snapshot, name)?;
            Ok(MetaOutcome::Continue)
        }
        _ => Err(CliError::Data(format!(
            "unknown console command {command:?}; type \\help"
        ))),
    }
}

fn add_history(editor: &mut Option<ConsoleEditor>, entry: &str) {
    if let Some(editor) = editor
        && !entry.is_empty()
    {
        if let Err(error) = editor.add_history_entry(entry) {
            eprintln!(
                "warning: {}",
                escape_field(&terminal_error("cannot update console history", error).to_string())
            );
        }
    }
}

fn terminal_error(context: &str, error: ReadlineError) -> CliError {
    CliError::Terminal(format!("{context}: {error}"))
}

fn format_duration(duration: Duration) -> String {
    if duration.as_micros() == 0 {
        format!("{} ns", duration.as_nanos())
    } else if duration.as_millis() == 0 {
        format!("{} µs", duration.as_micros())
    } else {
        format!("{:.3} ms", duration.as_secs_f64() * 1_000.0)
    }
}

fn timing_line(phase: &str, duration: Duration) -> String {
    format!("{phase}: {}", format_duration(duration))
}

fn print_tables(output: &mut impl Write, snapshot: &MetadataSnapshot) -> io::Result<()> {
    let mut rows: Vec<Vec<String>> = snapshot
        .objects()
        .iter()
        .filter_map(|object| {
            Some(vec![
                object.kind?.as_str().to_owned(),
                object.name.clone().unwrap_or_default(),
                object.guid.to_string(),
                object.physical_table.clone()?,
                yes_no(object.declared).to_owned(),
                yes_no(object.live).to_owned(),
            ])
        })
        .collect();
    rows.sort_by(|left, right| (&left[0], &left[1]).cmp(&(&right[0], &right[1])));
    print_table(
        output,
        &["Kind", "Name", "GUID", "Table", "Schema", "Live"],
        &rows,
    )?;
    writeln!(output, "({} objects)", rows.len())
}

/// The names of the temporary tables a statement can read.
fn temporary_table_names(temporary: &TempTablesManager) -> Vec<String> {
    temporary
        .tables()
        .map(|table| table.name().to_owned())
        .collect()
}

/// Prints the temporary tables of this session with their columns.
fn print_temporary_tables(
    output: &mut impl Write,
    temporary: &TempTablesManager,
) -> io::Result<()> {
    if temporary.is_empty() {
        return writeln!(output, "No temporary tables placed.");
    }
    let width = temporary
        .tables()
        .map(|table| table.name().chars().count())
        .max()
        .unwrap_or(0);
    for table in temporary.tables() {
        let columns = table
            .columns()
            .iter()
            .map(|column| {
                format!(
                    "{} [{}]",
                    escape_field(&column.label),
                    column_kind_label(&column.kind)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "{:<width$}  {columns}",
            escape_field(table.name()),
            width = width
        )?;
    }
    Ok(())
}

/// A short display name for a column kind, used by `\tables`.
fn column_kind_label(kind: &ColumnKind) -> String {
    match kind {
        ColumnKind::String { .. } => "String".to_owned(),
        ColumnKind::Number { .. } => "Number".to_owned(),
        ColumnKind::Boolean => "Boolean".to_owned(),
        ColumnKind::DateTime => "DateTime".to_owned(),
        ColumnKind::Reference { runtime_typed, .. } => {
            if *runtime_typed {
                "Reference*".to_owned()
            } else {
                "Reference".to_owned()
            }
        }
        ColumnKind::Binary { .. } => "Binary".to_owned(),
        ColumnKind::Uuid => "UUID".to_owned(),
        ColumnKind::Null => "Null".to_owned(),
        _ => "Unknown".to_owned(),
    }
}

fn print_indexes(output: &mut impl Write, snapshot: &MetadataSnapshot) -> io::Result<()> {
    let mut rows: Vec<Vec<String>> = snapshot
        .indexes()
        .iter()
        .map(|index| {
            vec![
                object_display_name(object_for_table(snapshot, &index.table)),
                index.table.clone(),
                index.declared_name.clone(),
                index.live_name.clone().unwrap_or_default(),
                index.logical_key.join(", "),
                yes_no(index.live_name.is_some() && index.unique_matches).to_owned(),
            ]
        })
        .collect();
    rows.sort_by(|left, right| (&left[0], &left[2]).cmp(&(&right[0], &right[2])));
    print_table(
        output,
        &["Metadata", "Table", "Declared", "Live", "Key", "Match"],
        &rows,
    )?;
    writeln!(output, "({} indexes)", rows.len())
}

fn print_description(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
    name: &str,
) -> Result<(), CliError> {
    let object =
        find_metadata_object(snapshot, name).map_err(|error| CliError::Data(error.to_string()))?;
    let fields =
        queryable_fields(snapshot, object).map_err(|error| CliError::Data(error.to_string()))?;
    writeln!(
        output,
        "{}  GUID={}  table={}  schema={}  live={}",
        bounded_field(&object_display_name(Some(object)), MAX_CELL_WIDTH),
        object.guid,
        bounded_field(
            object.physical_table.as_deref().unwrap_or(""),
            MAX_CELL_WIDTH
        ),
        yes_no(object.declared),
        yes_no(object.live),
    )
    .map_err(CliError::standard_output)?;

    let field_rows: Vec<Vec<String>> = fields
        .into_iter()
        .map(|field| {
            let origin = field
                .schema_name
                .strip_prefix("Fld")
                .and_then(|number| number.parse::<u32>().ok())
                .and_then(|number| {
                    snapshot
                        .fields()
                        .iter()
                        .find(|metadata| metadata.number == number)
                })
                .and_then(|metadata| metadata.extension_origin.clone())
                .unwrap_or_default();
            vec![
                field.name,
                field.schema_name,
                field.aliases.join(", "),
                field
                    .columns
                    .into_iter()
                    .map(|column| format!("{}:{}", column.physical_name, column.data_type))
                    .collect::<Vec<_>>()
                    .join(", "),
                field.reference_target.unwrap_or_default(),
                origin,
            ]
        })
        .collect();
    writeln!(output, "Attributes:").map_err(CliError::standard_output)?;
    print_table(
        output,
        &[
            "Name",
            "Schema name",
            "Aliases",
            "Physical members",
            "Reference target",
            "Extension",
        ],
        &field_rows,
    )
    .map_err(CliError::standard_output)?;

    let table = object.physical_table.as_deref().unwrap_or("");
    let index_rows: Vec<Vec<String>> = snapshot
        .indexes()
        .iter()
        .filter(|index| index.table.eq_ignore_ascii_case(table))
        .map(|index| {
            vec![
                index.declared_name.clone(),
                index.live_name.clone().unwrap_or_default(),
                index.logical_key.join(", "),
                yes_no(index.live_name.is_some() && index.unique_matches).to_owned(),
            ]
        })
        .collect();
    writeln!(output, "Indexes:").map_err(CliError::standard_output)?;
    print_table(output, &["Declared", "Live", "Key", "Match"], &index_rows)
        .map_err(CliError::standard_output)?;
    Ok(())
}

fn object_for_table<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    table: &str,
) -> Option<&'snapshot MetadataObject> {
    snapshot.objects().iter().find(|object| {
        object
            .physical_table
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(table))
    })
}

fn object_display_name(object: Option<&MetadataObject>) -> String {
    let Some(object) = object else {
        return String::new();
    };
    match (object.kind, object.name.as_deref()) {
        (Some(kind), Some(name)) => format!("{}.{name}", kind.as_str()),
        (_, Some(name)) => name.to_owned(),
        _ => String::new(),
    }
}

fn validate_query_rows(compiled: &CompiledQuery, rows: &QueryRows) -> Result<(), CliError> {
    for row in rows {
        if row.len() != compiled.columns.len() {
            return Err(CliError::Data(format!(
                "database returned {} columns, expected {}",
                row.len(),
                compiled.columns.len()
            )));
        }
    }
    Ok(())
}

fn print_query_rows(
    output: &mut impl Write,
    compiled: &CompiledQuery,
    rows: &QueryRows,
) -> io::Result<()> {
    let headers: Vec<&str> = compiled
        .columns
        .iter()
        .map(|column| column.label.as_str())
        .collect();
    print_table(output, &headers, rows)?;
    writeln!(output, "({} rows)", rows.len())
}

trait TableRow {
    fn cell(&self, index: usize) -> Cow<'_, str>;
}

impl TableRow for Vec<String> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.get(index).map_or("", String::as_str))
    }
}

impl TableRow for Vec<Cell> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        self.get(index).map_or(Cow::Borrowed(""), Cell::render)
    }
}

struct HeaderRow<'a>(&'a [&'a str]);

impl TableRow for HeaderRow<'_> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.0.get(index).copied().unwrap_or(""))
    }
}

fn print_table<R: TableRow>(
    output: &mut impl Write,
    headers: &[&str],
    rows: &[R],
) -> io::Result<()> {
    print_table_with_width(output, headers, rows, detected_table_width())
}

fn print_table_with_width<R: TableRow>(
    output: &mut impl Write,
    headers: &[&str],
    rows: &[R],
    terminal_width: Option<usize>,
) -> io::Result<()> {
    let mut widths = headers
        .iter()
        .map(|header| display_width(header).clamp(1, MAX_CELL_WIDTH))
        .collect::<Vec<_>>();
    for row in rows.iter().take(MAX_PRINTED_ROWS) {
        for (index, width) in widths.iter_mut().enumerate() {
            *width = (*width)
                .max(display_width(&row.cell(index)))
                .min(MAX_CELL_WIDTH);
        }
    }
    let omitted_columns = terminal_width.map_or(0, |terminal_width| {
        fit_table_widths(&mut widths, terminal_width)
    });
    if widths.is_empty() {
        if omitted_columns != 0 {
            writeln!(output, "({omitted_columns} columns omitted)")?;
        }
        return Ok(());
    }

    write_table_row(output, &HeaderRow(headers), &widths)?;
    for (index, width) in widths.iter().enumerate() {
        if index != 0 {
            output.write_all(b"-+-")?;
        }
        output.write_all("-".repeat(*width).as_bytes())?;
    }
    writeln!(output)?;
    for row in rows.iter().take(MAX_PRINTED_ROWS) {
        write_table_row(output, row, &widths)?;
    }
    if omitted_columns != 0 {
        writeln!(output, "({omitted_columns} columns omitted)")?;
    }
    let omitted = rows.len().saturating_sub(MAX_PRINTED_ROWS);
    if omitted != 0 {
        writeln!(output, "({omitted} rows omitted)")?;
    }
    Ok(())
}

fn fit_table_widths(widths: &mut Vec<usize>, terminal_width: usize) -> usize {
    let original_columns = widths.len();
    if terminal_width == 0 {
        widths.clear();
        return original_columns;
    }
    while widths.len() > 1 && widths.len() * 4 - 3 > terminal_width {
        widths.pop();
    }
    let separators = widths.len().saturating_sub(1) * 3;
    let available = terminal_width.saturating_sub(separators);
    while widths.iter().sum::<usize>() > available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > 1)
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        widths[index] -= 1;
    }
    original_columns - widths.len()
}

fn write_table_row(
    output: &mut impl Write,
    values: &impl TableRow,
    widths: &[usize],
) -> io::Result<()> {
    for (index, width) in widths.iter().enumerate() {
        if index != 0 {
            output.write_all(b" | ")?;
        }
        let value = bounded_field(&values.cell(index), *width);
        let padding = width.saturating_sub(UnicodeWidthStr::width(value.as_str()));
        output.write_all(value.as_bytes())?;
        output.write_all(" ".repeat(padding).as_bytes())?;
    }
    writeln!(output)
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(escape_field(value).as_str())
}

fn detected_table_width() -> Option<usize> {
    if !io::stdout().is_terminal() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        terminal_size().map(|(_, columns)| usize::from(columns))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::var("COLUMNS")
            .ok()
            .and_then(|columns| columns.parse().ok())
    }
}

fn statement_is_complete(source: &str) -> bool {
    let mut characters = source.chars().peekable();
    let mut string = false;
    let mut comment = false;
    let mut last_significant = None;
    while let Some(character) = characters.next() {
        if comment {
            if character == '\n' {
                comment = false;
            }
            continue;
        }
        if string {
            if character == '"' {
                if characters.peek() == Some(&'"') {
                    characters.next();
                } else {
                    string = false;
                }
            }
            continue;
        }
        match character {
            '"' => string = true,
            '/' if characters.peek() == Some(&'/') => {
                characters.next();
                comment = true;
            }
            value if !value.is_whitespace() => last_significant = Some(value),
            _ => {}
        }
    }
    !string && last_significant == Some(';')
}

fn decode_input_line(line: &[u8]) -> Result<&str, usize> {
    std::str::from_utf8(line).map_err(|error| error.valid_up_to())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundedLine {
    Read(usize),
    TooLong,
}

async fn read_bounded_line(
    input: &mut (impl AsyncBufRead + Unpin),
    output: &mut Vec<u8>,
    limit: usize,
) -> io::Result<BoundedLine> {
    let mut too_long = false;
    loop {
        let buffer = input.fill_buf().await?;
        if buffer.is_empty() {
            return Ok(if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Read(output.len())
            });
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |position| position + 1);
        if !too_long {
            let retained = consumed.min(limit.saturating_add(1).saturating_sub(output.len()));
            output.extend_from_slice(&buffer[..retained]);
            too_long = output.len() > limit;
        }
        input.consume(consumed);
        if newline.is_some() {
            return Ok(if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Read(output.len())
            });
        }
    }
}

#[cfg(any(target_os = "linux", test))]
fn footer_text(columns: u16) -> String {
    let available = usize::from(columns.saturating_sub(1));
    if COMMAND_HINT.len() <= available {
        return COMMAND_HINT.to_owned();
    }
    if available <= 3 {
        return ".".repeat(available);
    }
    format!("{}...", &COMMAND_HINT[..available - 3])
}

#[cfg(target_os = "linux")]
struct PinnedFooter {
    enabled: bool,
    active: bool,
    rows: u16,
    columns: u16,
}

#[cfg(target_os = "linux")]
impl PinnedFooter {
    fn enable(interactive: bool) -> Result<Self, CliError> {
        let mut footer = Self {
            enabled: interactive && io::stdout().is_terminal(),
            active: false,
            rows: 0,
            columns: 0,
        };
        if footer.enabled {
            footer.redraw()?;
        }
        Ok(footer)
    }

    fn redraw(&mut self) -> Result<(), CliError> {
        if !self.enabled {
            return Ok(());
        }
        let Some((rows, columns)) = terminal_size() else {
            return Ok(());
        };
        if rows < 3 || columns < 4 {
            self.restore()?;
            return Ok(());
        }

        if !self.active || self.rows != rows || self.columns != columns {
            self.restore()?;
            self.rows = rows;
            self.columns = columns;
            self.active = true;
            let hint = footer_text(columns);
            write_terminal(format_args!(
                "\x1b[1;{}r\x1b[{};1H\x1b[2K\x1b[2m{}\x1b[0m\x1b[{};1H",
                rows - 1,
                rows,
                hint,
                rows - 1
            ))?;
        } else {
            let hint = footer_text(columns);
            write_terminal(format_args!(
                "\x1b7\x1b[{};1H\x1b[2K\x1b[2m{}\x1b[0m\x1b8",
                rows, hint
            ))?;
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), CliError> {
        if self.active {
            write_terminal(format_args!("\x1b[r\x1b[{};1H\x1b[2K", self.rows))?;
            self.active = false;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for PinnedFooter {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(target_os = "linux")]
fn terminal_size() -> Option<(u16, u16)> {
    let mut size = MaybeUninit::<libc::winsize>::uninit();
    // SAFETY: `size` is writable storage for `winsize`, and stdout was
    // verified to be an interactive terminal before this function is used.
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: a successful TIOCGWINSZ call initialized the complete value.
    let size = unsafe { size.assume_init() };
    (size.ws_row > 0 && size.ws_col > 0).then_some((size.ws_row, size.ws_col))
}

#[cfg(target_os = "linux")]
fn write_terminal(arguments: std::fmt::Arguments<'_>) -> Result<(), CliError> {
    let mut output = io::stdout().lock();
    output
        .write_fmt(arguments)
        .and_then(|()| output.flush())
        .map_err(|error| CliError::Io("cannot update terminal footer".to_owned(), error))
}

#[cfg(not(target_os = "linux"))]
struct PinnedFooter;

#[cfg(not(target_os = "linux"))]
impl PinnedFooter {
    fn enable(_interactive: bool) -> Result<Self, CliError> {
        Ok(Self)
    }

    fn redraw(&mut self) -> Result<(), CliError> {
        Ok(())
    }

    fn restore(&mut self) -> Result<(), CliError> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
struct TerminalUtf8Guard {
    original: Option<libc::termios>,
}

#[cfg(target_os = "linux")]
impl TerminalUtf8Guard {
    fn enable(interactive: bool) -> Result<Self, CliError> {
        if !interactive {
            return Ok(Self { original: None });
        }

        let mut original = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: `original` points to writable storage for a complete termios
        // value, and STDIN_FILENO is valid for this process.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, original.as_mut_ptr()) } != 0 {
            return Err(CliError::Io(
                "cannot inspect terminal input settings".to_owned(),
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: tcgetattr returned success and initialized the value.
        let original = unsafe { original.assume_init() };
        if original.c_iflag & libc::IUTF8 != 0 {
            return Ok(Self { original: None });
        }

        let mut updated = original;
        updated.c_iflag |= libc::IUTF8;
        // SAFETY: `updated` is a valid termios value obtained from this stdin
        // terminal with only the documented IUTF8 input bit changed.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &updated) } != 0 {
            return Err(CliError::Io(
                "cannot enable UTF-8 terminal input".to_owned(),
                io::Error::last_os_error(),
            ));
        }
        Ok(Self {
            original: Some(original),
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for TerminalUtf8Guard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            // SAFETY: this is the complete termios value read from stdin by
            // `enable`; restoration is best-effort during scope cleanup.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, original);
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct TerminalUtf8Guard;

#[cfg(not(target_os = "linux"))]
impl TerminalUtf8Guard {
    fn enable(_interactive: bool) -> Result<Self, CliError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::{HashMap, HashSet};
    use std::time::Duration;

    use open_sdbl::metadata::{
        FieldId, LiveTable, MetadataKind, ObjectId, SchemaStorage, StandardFieldId, parse_db_names,
        resolve_metadata,
    };
    use open_sdbl::query::{
        CompileOptions, PostgresBackend, PresentationExpression, QueryCompiler,
    };
    use rustyline::highlight::Highlighter;
    use unicode_width::UnicodeWidthStr;

    use super::{
        BoundedLine, CONSOLE_HELP, ColumnKind, CompletionPath, ConsoleHelper, TempTablesManager,
        UNRESOLVED_REFERENCE, column_kind_label, completion_start, decode_input_line,
        default_presentation_template, display_width, ensure_session_remains_usable, footer_text,
        format_duration, presentation_plan, print_table_with_width, print_temporary_tables,
        push_service_table_candidates, push_unique, push_virtual_table_candidates,
        read_bounded_line, resolved_presentation, split_deferred_payload, statement_is_complete,
        timing_line,
    };
    use crate::{MAX_CELL_WIDTH, MAX_PRINTED_ROWS};

    #[test]
    fn recognizes_multiline_termination_outside_strings_and_comments() {
        assert!(!statement_is_complete("ВЫБРАТЬ Код\n"));
        assert!(statement_is_complete("ВЫБРАТЬ Код\nИЗ Справочник.Тест;\n"));
        assert!(!statement_is_complete("ВЫБРАТЬ \"text;\"\n"));
        assert!(statement_is_complete("ВЫБРАТЬ Код; // done\n"));
        assert!(!statement_is_complete("ВЫБРАТЬ Код // ;\n"));
    }

    #[test]
    fn validates_cyrillic_bytes_without_terminating_the_reader() {
        assert_eq!(
            decode_input_line("select Код Из Справочник.Договоры;\n".as_bytes()).unwrap(),
            "select Код Из Справочник.Договоры;\n"
        );

        let mut damaged = "select Код".as_bytes().to_vec();
        damaged.pop();
        damaged.extend_from_slice(b";\n");
        assert_eq!(decode_input_line(&damaged), Err(damaged.len() - 3));
    }

    #[tokio::test]
    async fn bounds_non_interactive_input_lines_and_drains_the_remainder() {
        let source = b"123456789\nnext\n";
        let mut input = tokio::io::BufReader::new(&source[..]);
        let mut line = Vec::new();
        assert_eq!(
            read_bounded_line(&mut input, &mut line, 4).await.unwrap(),
            BoundedLine::TooLong
        );
        assert_eq!(line.len(), 5);
        line.clear();
        assert_eq!(
            read_bounded_line(&mut input, &mut line, 8).await.unwrap(),
            BoundedLine::Read(5)
        );
        assert_eq!(line, b"next\n");
    }

    #[test]
    fn exits_after_an_error_when_the_database_session_is_dead() {
        assert!(ensure_session_remains_usable(true).is_err());
        assert!(ensure_session_remains_usable(false).is_ok());
    }

    #[test]
    fn splits_binary_deferred_payloads() {
        let mut payload = vec![0, 0, 0, 0xea];
        payload.extend_from_slice(&[7; 16]);
        assert_eq!(
            split_deferred_payload(&payload).unwrap(),
            Some((0xea, [7; 16]))
        );
        assert_eq!(split_deferred_payload(&[0; 20]).unwrap(), None);
        assert!(split_deferred_payload(&[0; 16]).is_err());
        assert!(split_deferred_payload(&[]).is_err());
    }

    #[test]
    fn formats_sql_generation_duration_compactly() {
        assert_eq!(format_duration(Duration::from_nanos(750)), "750 ns");
        assert_eq!(format_duration(Duration::from_micros(42)), "42 µs");
        assert_eq!(format_duration(Duration::from_micros(1_250)), "1.250 ms");
        assert_eq!(
            timing_line("PostgreSQL execution", Duration::from_micros(42)),
            "PostgreSQL execution: 42 µs"
        );
    }

    #[test]
    fn aligns_cjk_by_display_columns() {
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("\x1b"), "\\u{1b}".len());
        let rows = vec![
            vec!["界".to_owned(), "x".to_owned()],
            vec!["a".to_owned(), "y".to_owned()],
        ];
        let mut output = Vec::new();
        print_table_with_width(&mut output, &["A", "B"], &rows, None).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "A  | B\n---+--\n界 | x\na  | y\n"
        );
    }

    #[test]
    fn bounds_table_rows_cells_and_terminal_width() {
        let rows = (0..MAX_PRINTED_ROWS + 2)
            .map(|_| vec!["界".repeat(MAX_CELL_WIDTH), "value".to_owned()])
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        print_table_with_width(&mut output, &["Wide", "Value"], &rows, Some(24)).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("…"));
        assert!(output.ends_with("(2 rows omitted)\n"));
        for line in output.lines().take(MAX_PRINTED_ROWS + 2) {
            assert!(UnicodeWidthStr::width(line) <= 24, "{line:?}");
        }

        let headers = ["A", "B", "C", "D", "E", "F"];
        let mut output = Vec::new();
        print_table_with_width(&mut output, &headers, &[vec!["x".to_owned(); 6]], Some(8)).unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("(4 columns omitted)")
        );
    }

    #[test]
    fn completes_commands_keywords_and_cyrillic_metadata_case_insensitively() {
        let helper = ConsoleHelper::for_test(
            vec![
                "\\refresh".to_owned(),
                "ВЫБРАТЬ".to_owned(),
                "Справочник.Договоры".to_owned(),
                "Организация.Код".to_owned(),
            ],
            vec!["Справочник.Договоры".to_owned()],
            HashSet::new(),
        );

        let (_, commands) = helper.complete_values("\\REF", "\\REF".len());
        assert_eq!(commands[0].replacement, "\\refresh");
        let (_, objects) = helper.complete_values("из справ", "из справ".len());
        assert_eq!(objects[0].replacement, "Справочник.Договоры");
        let (_, fields) = helper.complete_values("Организация.к", "Организация.к".len());
        assert_eq!(fields[0].replacement, "Организация.Код");
    }

    #[test]
    fn candidate_deduplication_keeps_the_first_case_insensitive_spelling() {
        let mut candidates = vec!["Код".to_owned()];
        let mut keys = HashSet::from(["код".to_owned()]);
        push_unique(&mut candidates, &mut keys, "КОД");
        push_unique(&mut candidates, &mut keys, "Description");
        push_unique(&mut candidates, &mut keys, "description");
        assert_eq!(candidates, ["Код", "Description"]);
    }

    #[test]
    fn stores_reference_completion_aliases_linearly_and_expands_only_a_typed_path() {
        let path = CompletionPath::new(
            vec!["Организация".to_owned(), "Organization".to_owned()],
            vec![
                "Код".to_owned(),
                "Code".to_owned(),
                "Description".to_owned(),
            ],
        );
        assert_eq!(path.stored_names(), 2 + 3);
        assert_ne!(path.stored_names(), 2 * 3);
        let mut helper = ConsoleHelper::for_test(Vec::new(), Vec::new(), HashSet::new());
        helper.paths.push(path);

        let (_, empty) = helper.complete_values("SELECT ", "SELECT ".len());
        assert!(empty.is_empty());
        let source = "SELECT Организация.к";
        let (_, values) = helper.complete_values(source, source.len());
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].replacement, "Организация.Код");
    }

    #[test]
    fn unresolved_deferred_presentations_are_visible() {
        let object = ObjectId::from_bytes([7; 16]);
        assert_eq!(
            resolved_presentation(&HashMap::new(), object, [9; 16]),
            UNRESOLVED_REFERENCE
        );
    }

    #[test]
    fn completes_virtual_tables_by_resolved_register_kind() {
        let accumulation_names = [
            "Остатки".to_owned(),
            "AccumulationRegister.Остатки".to_owned(),
            "РегистрНакопления.Остатки".to_owned(),
        ];
        let information_names = [
            "Цены".to_owned(),
            "InformationRegister.Цены".to_owned(),
            "РегистрСведений.Цены".to_owned(),
        ];
        let mut candidates = Vec::new();
        let mut candidate_keys = HashSet::new();
        push_virtual_table_candidates(
            &mut candidates,
            &mut candidate_keys,
            MetadataKind::AccumulationRegister,
            &accumulation_names,
        );
        push_virtual_table_candidates(
            &mut candidates,
            &mut candidate_keys,
            MetadataKind::InformationRegister,
            &information_names,
        );
        let helper = ConsoleHelper::for_test(candidates.clone(), candidates, HashSet::new());

        let russian = "из регистрнакопления.остатки.ос";
        let (_, values) = helper.complete_values(russian, russian.len());
        assert_eq!(values[0].replacement, "РегистрНакопления.Остатки.Остатки()");
        let english = "FROM AccumulationRegister.Остатки.ba";
        let (_, values) = helper.complete_values(english, english.len());
        assert_eq!(
            values[0].replacement,
            "AccumulationRegister.Остатки.Balance()"
        );
        let slice = "ИЗ РегистрСведений.Цены.срезп";
        let (_, values) = helper.complete_values(slice, slice.len());
        assert_eq!(values.len(), 2);
        assert!(
            values
                .iter()
                .any(|value| value.replacement.ends_with("СрезПервых()"))
        );
        assert!(
            values
                .iter()
                .any(|value| value.replacement.ends_with("СрезПоследних()"))
        );
    }

    #[test]
    fn completes_service_sources_under_their_owners() {
        let mut candidates = Vec::new();
        let mut keys = HashSet::new();
        push_service_table_candidates(
            &mut candidates,
            &mut keys,
            MetadataKind::AccumulationRegister,
            &["РегистрНакопления.RegisteredTotals".to_owned()],
            true,
        );
        push_service_table_candidates(
            &mut candidates,
            &mut keys,
            MetadataKind::ChartOfCalculationTypes,
            &["ChartOfCalculationTypes.Payroll".to_owned()],
            false,
        );
        push_service_table_candidates(
            &mut candidates,
            &mut keys,
            MetadataKind::ChartOfAccounts,
            &["ChartOfAccounts.Main".to_owned()],
            false,
        );

        assert!(candidates.contains(&"РегистрНакопления.RegisteredTotals.Изменения".to_owned()));
        assert!(
            candidates
                .contains(&"ChartOfCalculationTypes.Payroll.LeadingCalculationKinds".to_owned())
        );
        assert!(candidates.contains(&"ChartOfAccounts.Main.ExtraDimensions".to_owned()));
    }

    #[test]
    fn restricts_source_completion_to_the_qualified_metadata_hierarchy() {
        let helper = ConsoleHelper::for_test(
            vec![
                "Код".to_owned(),
                "Договоры".to_owned(),
                "_Референс42".to_owned(),
                "Организация.Код".to_owned(),
            ],
            vec![
                "Catalog.Contracts".to_owned(),
                "Document.Sale".to_owned(),
                "РегистрНакопления.Остатки".to_owned(),
                "РегистрНакопления.Остатки.Остатки()".to_owned(),
                "Справочник.Договоры".to_owned(),
            ],
            HashSet::new(),
        );

        let (_, empty_source) = helper.complete_values("FROM ", "FROM ".len());
        assert_eq!(empty_source.len(), 4);
        assert!(
            empty_source
                .iter()
                .all(|candidate| candidate.replacement.matches('.').count() == 1)
        );
        assert!(empty_source.iter().all(|candidate| {
            !matches!(
                candidate.replacement.as_str(),
                "Код" | "Договоры" | "_Референс42"
            )
        }));

        let (_, catalogs) = helper.complete_values("из спр", "из спр".len());
        assert_eq!(catalogs.len(), 1);
        assert_eq!(catalogs[0].replacement, "Справочник.Договоры");

        let virtual_prefix = "JOIN РегистрНакопления.Остатки.ос";
        let (_, virtual_sources) = helper.complete_values(virtual_prefix, virtual_prefix.len());
        assert_eq!(virtual_sources.len(), 1);
        assert_eq!(
            virtual_sources[0].replacement,
            "РегистрНакопления.Остатки.Остатки()"
        );

        let (_, fields) =
            helper.complete_values("ВЫБРАТЬ Организация.к", "ВЫБРАТЬ Организация.к".len());
        assert_eq!(fields[0].replacement, "Организация.Код");
    }

    #[test]
    fn does_not_attach_virtual_tables_to_catalogs() {
        let mut candidates = Vec::new();
        let mut candidate_keys = HashSet::new();
        push_virtual_table_candidates(
            &mut candidates,
            &mut candidate_keys,
            MetadataKind::Catalog,
            &["Справочник.Номенклатура".to_owned()],
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn lists_and_completes_session_temporary_tables() {
        assert!(CONSOLE_HELP.contains("\\tables"));
        assert!(CONSOLE_HELP.contains("ПОМЕСТИТЬ"));

        let mut empty = Vec::new();
        print_temporary_tables(&mut empty, &TempTablesManager::new()).unwrap();
        assert_eq!(
            String::from_utf8(empty).unwrap(),
            "No temporary tables placed.\n"
        );

        let mut helper = ConsoleHelper::for_test(
            vec!["\\tables".to_owned()],
            vec!["Справочник.Договоры".to_owned()],
            HashSet::new(),
        );
        helper.set_temporary_tables(vec!["Обороты".to_owned(), "Остатки".to_owned()]);

        let (_, sources) = helper.complete_values("ИЗ ", "ИЗ ".len());
        assert_eq!(
            sources
                .iter()
                .map(|candidate| candidate.replacement.as_str())
                .collect::<Vec<_>>(),
            ["Обороты", "Остатки", "Справочник.Договоры"]
        );

        let (_, filtered) = helper.complete_values("ИЗ обо", "ИЗ обо".len());
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].replacement, "Обороты");

        // Temporary tables are sources, not expression candidates.
        let (_, projection) = helper.complete_values("ВЫБРАТЬ обо", "ВЫБРАТЬ обо".len());
        assert!(projection.is_empty());

        let (_, commands) = helper.complete_values("\\tab", "\\tab".len());
        assert_eq!(commands[0].replacement, "\\tables");
    }

    #[test]
    fn lists_temporary_tables_placed_in_this_session() {
        let serialized = b"{1,{b8bac76b-c91b-4d78-8a70-ffa39f8de694,\"Reference\",53}}";
        let length = u16::try_from(serialized.len()).unwrap();
        let mut compressed = vec![1];
        compressed.extend_from_slice(&length.to_le_bytes());
        compressed.extend_from_slice(&(!length).to_le_bytes());
        compressed.extend_from_slice(serialized);
        let snapshot = resolve_metadata(
            parse_db_names(&compressed).unwrap(),
            Vec::new(),
            SchemaStorage {
                tables: Vec::new(),
                anomalies: Vec::new(),
            },
            Vec::<LiveTable>::new(),
        )
        .snapshot;

        let mut temporary = TempTablesManager::new();
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_batch(
                "ВЫБРАТЬ 1 КАК Итог, ИСТИНА КАК Флаг ПОМЕСТИТЬ Обороты;",
                &CompileOptions::new(),
                &mut temporary,
            )
            .unwrap();

        let mut listing = Vec::new();
        print_temporary_tables(&mut listing, &temporary).unwrap();
        assert_eq!(
            String::from_utf8(listing).unwrap(),
            "Обороты  Итог [Number], Флаг [Boolean]\n"
        );
    }

    #[test]
    fn labels_temporary_table_column_kinds() {
        assert_eq!(
            column_kind_label(&ColumnKind::String { length: Some(9) }),
            "String"
        );
        assert_eq!(
            column_kind_label(&ColumnKind::Reference {
                targets: Vec::new(),
                runtime_typed: true,
            }),
            "Reference*"
        );
        assert_eq!(column_kind_label(&ColumnKind::DateTime), "DateTime");
        assert_eq!(
            column_kind_label(&ColumnKind::Unknown {
                data_type: "bytea".to_owned(),
            }),
            "Unknown"
        );
    }

    #[test]
    fn finds_completion_boundary_without_splitting_utf8_or_dotted_names() {
        let line = "ВЫБРАТЬ Организация.Ко";
        assert_eq!(completion_start(line, line.len()), "ВЫБРАТЬ ".len());
        assert_eq!(completion_start("\\d Спр", "\\d Спр".len()), "\\d ".len());
    }

    #[test]
    fn highlights_lexer_tokens_without_changing_display_text() {
        let helper = ConsoleHelper::for_test(
            Vec::new(),
            Vec::new(),
            HashSet::from(["договоры".to_owned()]),
        );
        let line = "ВЫБРАТЬ Договоры // test";
        let Cow::Owned(highlighted) = helper.highlight(line, line.len()) else {
            panic!("expected styled output");
        };
        assert!(highlighted.contains("\x1b[1;34mВЫБРАТЬ\x1b[0m"));
        assert!(highlighted.contains("\x1b[36mДоговоры\x1b[0m"));
        assert!(highlighted.contains("\x1b[2;37m// test\x1b[0m"));
        let plain = highlighted
            .replace("\x1b[1;34m", "")
            .replace("\x1b[36m", "")
            .replace("\x1b[2;37m", "")
            .replace("\x1b[0m", "");
        assert_eq!(plain, line);
    }

    #[test]
    fn footer_fits_the_last_terminal_row() {
        assert_eq!(footer_text(1), "");
        assert_eq!(footer_text(4), "...");
        assert!(footer_text(24).len() < 24);
        assert_eq!(footer_text(200), super::COMMAND_HINT);
    }

    #[test]
    fn catalog_default_presentation_is_description_space_code() {
        let description = FieldId::Standard(StandardFieldId::Description);
        let code = FieldId::Standard(StandardFieldId::Code);
        let (fields, expression) = default_presentation_template(
            Some(MetadataKind::Catalog),
            "Номенклатура",
            Some(description),
            Some(code),
            None,
            None,
            None,
        );
        assert_eq!(fields, [description, code]);
        assert_eq!(
            expression,
            PresentationExpression::Concat(vec![
                PresentationExpression::Field(description),
                PresentationExpression::Literal(" (".to_owned()),
                PresentationExpression::Field(code),
                PresentationExpression::Literal(")".to_owned()),
            ])
        );
    }

    #[test]
    fn document_default_presentation_is_type_number_and_period() {
        let number = FieldId::Standard(StandardFieldId::Number);
        let date = FieldId::Standard(StandardFieldId::Date);
        let (fields, expression) = default_presentation_template(
            Some(MetadataKind::Document),
            "Реализация товаров",
            None,
            None,
            Some(number),
            Some(date),
            None,
        );
        assert_eq!(fields, [number, date]);
        assert_eq!(
            expression,
            PresentationExpression::Concat(vec![
                PresentationExpression::Literal("Реализация товаров".to_owned()),
                PresentationExpression::Literal(" ".to_owned()),
                PresentationExpression::Field(number),
                PresentationExpression::Literal(" от ".to_owned()),
                PresentationExpression::Field(date),
            ])
        );
    }

    #[test]
    fn presentation_cache_uses_the_production_lookup_and_clears_on_refresh() {
        let serialized = b"{1,{b8bac76b-c91b-4d78-8a70-ffa39f8de694,\"Reference\",53}}";
        let length = u16::try_from(serialized.len()).unwrap();
        let mut compressed = vec![1];
        compressed.extend_from_slice(&length.to_le_bytes());
        compressed.extend_from_slice(&(!length).to_le_bytes());
        compressed.extend_from_slice(serialized);
        let snapshot = resolve_metadata(
            parse_db_names(&compressed).unwrap(),
            Vec::new(),
            SchemaStorage {
                tables: Vec::new(),
                anomalies: Vec::new(),
            },
            Vec::<LiveTable>::new(),
        )
        .snapshot;
        let mut cache = HashMap::new();
        let object = ObjectId::from_bytes([7; 16]);
        let first = presentation_plan(&mut cache, &snapshot, object);
        let second = presentation_plan(&mut cache, &snapshot, object);
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
        cache.clear();
        let refreshed = presentation_plan(&mut cache, &snapshot, object);
        assert_eq!(refreshed, first);
        assert_eq!(cache.len(), 1);
    }
}
