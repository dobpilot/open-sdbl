use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::{self, IsTerminal, Write};
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

use moka::future::Cache;
use open_sdbl::metadata::{MetadataKind, MetadataObject, MetadataSnapshot, ObjectId};
use open_sdbl::query::{
    CompiledQuery, PreparedMsSqlQuery, PreparedPostgresQuery, PresentationExpression,
    PresentationPlan, PresentationRequest, find_metadata_object,
    prepare_mssql_query_with_year_offset, prepare_postgres_query, queryable_field_catalog,
    queryable_fields,
};
use open_sdbl::{TokenKind, tokenize};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{CliError, DatabaseDialect, DatabaseSession, QueryRows, escape_field, yes_no};

const CONSOLE_HELP: &str = "Commands:
  \\dt                 list resolved metadata tables
  \\di                 list declared and live indexes
  \\d <metadata-name>  describe attributes and indexes
  \\refresh            reload DBNames, Config, SchemaStorage, and catalogs
  \\help               show this help
  \\q                  quit

Enter a supported 1C SELECT query and terminate it with a semicolon.
";

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
];

const PRESENTATION_POLICY_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PresentationPlanKey {
    metadata_generation: u64,
    object: ObjectId,
    language: &'static str,
    policy_version: u32,
}

#[derive(Debug, Clone)]
struct ConsoleHelper {
    candidates: Vec<String>,
    source_candidates: Vec<String>,
    known_identifiers: HashSet<String>,
}

impl ConsoleHelper {
    fn from_snapshot(snapshot: &MetadataSnapshot) -> Self {
        let mut candidates = ["\\dt", "\\di", "\\d", "\\refresh", "\\help", "\\q"]
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

        let fields_by_object = queryable_field_catalog(snapshot);
        let mut object_by_table = HashMap::new();
        for object in &snapshot.objects {
            let object_id = ObjectId::from(&object.guid);
            if let Some(table) = object.physical_table.as_deref() {
                object_by_table
                    .entry(normalize_physical_table(table))
                    .or_insert(object_id);
            }
        }

        for object in &snapshot.objects {
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
            if let Some(table) = object.physical_table.as_deref() {
                push_unique(&mut candidates, &mut candidate_keys, table);
            }

            let Some(fields) = fields_by_object.get(&ObjectId::from(&object.guid)) else {
                continue;
            };
            for field in fields {
                push_unique(&mut candidates, &mut candidate_keys, &field.name);
                for alias in &field.aliases {
                    push_unique(&mut candidates, &mut candidate_keys, alias);
                }
                for object_name in &object_names {
                    for alias in &field.aliases {
                        push_unique(
                            &mut candidates,
                            &mut candidate_keys,
                            &format!("{object_name}.{alias}"),
                        );
                    }
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
                for source_alias in &field.aliases {
                    for target_field in target_fields {
                        for target_alias in &target_field.aliases {
                            push_unique(
                                &mut candidates,
                                &mut candidate_keys,
                                &format!("{source_alias}.{target_alias}"),
                            );
                        }
                    }
                }
            }
        }

        candidates.sort_by_cached_key(|value| value.to_lowercase());
        source_candidates.sort_by_cached_key(|value| value.to_lowercase());
        let known_identifiers = candidates
            .iter()
            .flat_map(|candidate| candidate.split('.'))
            .filter(|part| !part.starts_with('\\'))
            .map(str::to_lowercase)
            .collect();
        Self {
            candidates,
            source_candidates,
            known_identifiers,
        }
    }

    fn complete_values(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let start = completion_start(line, pos);
        let prefix = line[start..pos].to_lowercase();
        let source_context = is_source_completion_context(line, start);
        let candidates = if source_context {
            &self.source_candidates
        } else {
            &self.candidates
        };
        let complete_virtual_source = prefix.bytes().filter(|byte| *byte == b'.').count() >= 2;
        let values = candidates
            .iter()
            .filter(|candidate| {
                !source_context
                    || complete_virtual_source
                    || candidate.bytes().filter(|byte| *byte == b'.').count() == 1
            })
            .filter(|candidate| candidate.to_lowercase().starts_with(&prefix))
            .map(|candidate| Pair {
                display: candidate.clone(),
                replacement: candidate.clone(),
            })
            .collect();
        (start, values)
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
    character == '\\' || character == '_' || character == '.' || character.is_alphanumeric()
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

fn normalize_physical_table(table: &str) -> String {
    table.trim_start_matches('_').to_lowercase()
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
    }
}

enum PreparedQuery {
    Postgres(PreparedPostgresQuery),
    MsSql(PreparedMsSqlQuery),
}

impl PreparedQuery {
    fn presentation_request(&self) -> &PresentationRequest {
        match self {
            Self::Postgres(query) => query.presentation_request(),
            Self::MsSql(query) => query.presentation_request(),
        }
    }

    fn compile(
        self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
    ) -> Result<CompiledQuery, open_sdbl::query::QueryDiagnostic> {
        match self {
            Self::Postgres(query) => query.compile(snapshot, plans),
            Self::MsSql(query) => query.compile(snapshot, plans),
        }
    }
}

pub(super) async fn run(
    session: &mut DatabaseSession,
    mut snapshot: MetadataSnapshot,
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
    let presentation_cache = Cache::builder()
        .max_capacity(1_024)
        .time_to_idle(Duration::from_secs(30 * 60))
        .time_to_live(Duration::from_secs(6 * 60 * 60))
        .build();
    let mut metadata_generation = 0_u64;

    if interactive {
        println!("open-sdbl 1C query console. Type \\help for help.");
    }
    let mut footer = PinnedFooter::enable(interactive)?;
    loop {
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
                    println!("^C");
                    statement.clear();
                    continue;
                }
                Err(ReadlineError::Eof) => 0,
                Err(error) => {
                    return Err(terminal_error("cannot read console input", error));
                }
            }
        } else {
            input
                .read_until(b'\n', &mut line)
                .await
                .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?
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
            add_history(&mut editor, line.trim())?;
            match execute_meta_command(session, &mut snapshot, line.trim()).await {
                Ok(MetaOutcome::Continue) => {}
                Ok(MetaOutcome::Refreshed) => {
                    metadata_generation = metadata_generation.wrapping_add(1);
                    if let Some(helper) = editor.as_mut().and_then(Editor::helper_mut) {
                        *helper = ConsoleHelper::from_snapshot(&snapshot);
                    }
                }
                Ok(MetaOutcome::Quit) => return Ok(()),
                Err(error) => eprintln!("error: {error}"),
            }
            continue;
        }

        statement.push_str(line);
        if !statement_is_complete(&statement) {
            continue;
        }

        add_history(&mut editor, statement.trim())?;
        let generation_started = Instant::now();
        let prepared = match session.dialect() {
            DatabaseDialect::Postgres => {
                prepare_postgres_query(&statement, &snapshot).map(PreparedQuery::Postgres)
            }
            DatabaseDialect::MsSql { year_offset } => {
                prepare_mssql_query_with_year_offset(&statement, &snapshot, year_offset)
                    .map(PreparedQuery::MsSql)
            }
        };
        let compilation = match prepared {
            Ok(prepared) => {
                let plans = presentation_plans(
                    &presentation_cache,
                    metadata_generation,
                    &snapshot,
                    prepared.presentation_request(),
                )
                .await;
                prepared.compile(&snapshot, &plans)
            }
            Err(error) => Err(error),
        };
        let generation_elapsed = generation_started.elapsed();
        match compilation {
            Ok(compiled) => {
                println!("{}", timing_line("SQL generation", generation_elapsed));
                println!("SQL: {}", compiled.sql);
                let execution_started = Instant::now();
                let execution = session.query(&compiled.sql, compiled.columns.len()).await;
                let execution_elapsed = execution_started.elapsed();
                match execution {
                    Ok(rows) => {
                        println!(
                            "{}",
                            timing_line(session.execution_label(), execution_elapsed)
                        );
                        if let Err(error) = print_query_rows(&compiled, &rows) {
                            eprintln!("error: {error}");
                        }
                    }
                    Err(error) => eprintln!(
                        "error: {error} ({}: {})",
                        session.execution_label(),
                        format_duration(execution_elapsed)
                    ),
                }
            }
            Err(error) => eprintln!(
                "error: {error} (SQL generation: {})",
                format_duration(generation_elapsed)
            ),
        }
        statement.clear();
    }
}

async fn presentation_plans(
    cache: &Cache<PresentationPlanKey, PresentationPlan>,
    metadata_generation: u64,
    snapshot: &MetadataSnapshot,
    request: &PresentationRequest,
) -> Vec<PresentationPlan> {
    let mut plans = Vec::with_capacity(request.targets.len());
    for target in &request.targets {
        let key = PresentationPlanKey {
            metadata_generation,
            object: target.object,
            language: "ru",
            policy_version: PRESENTATION_POLICY_VERSION,
        };
        let fallback = default_presentation_plan(snapshot, target.object);
        plans.push(cache.get_with(key, async move { fallback }).await);
    }
    plans
}

fn default_presentation_plan(snapshot: &MetadataSnapshot, object: ObjectId) -> PresentationPlan {
    let metadata_object = snapshot.object_by_id(object);
    let kind = metadata_object.and_then(|object| object.kind);
    let type_name = metadata_object.map_or_else(
        || "Документ".to_owned(),
        |metadata_object| {
            snapshot
                .descriptors
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
    command: &str,
) -> Result<MetaOutcome, CliError> {
    match command {
        "\\q" => Ok(MetaOutcome::Quit),
        "\\help" | "\\?" => {
            print!("{CONSOLE_HELP}");
            Ok(MetaOutcome::Continue)
        }
        "\\dt" => {
            print_tables(snapshot);
            Ok(MetaOutcome::Continue)
        }
        "\\di" => {
            print_indexes(snapshot);
            Ok(MetaOutcome::Continue)
        }
        "\\refresh" => {
            *snapshot = session.metadata().await?;
            println!("Metadata refreshed.");
            Ok(MetaOutcome::Refreshed)
        }
        _ if command == "\\d" => Err(CliError::Data(
            "usage: \\d <qualified-or-unique-metadata-name>".to_owned(),
        )),
        _ if command.starts_with("\\d ") || command.starts_with("\\d\t") => {
            let name = command[2..].trim();
            print_description(snapshot, name)?;
            Ok(MetaOutcome::Continue)
        }
        _ => Err(CliError::Data(format!(
            "unknown console command {command:?}; type \\help"
        ))),
    }
}

fn add_history(editor: &mut Option<ConsoleEditor>, entry: &str) -> Result<(), CliError> {
    if let Some(editor) = editor
        && !entry.is_empty()
    {
        editor
            .add_history_entry(entry)
            .map_err(|error| terminal_error("cannot update console history", error))?;
    }
    Ok(())
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

fn print_tables(snapshot: &MetadataSnapshot) {
    let mut rows: Vec<Vec<String>> = snapshot
        .objects
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
    print_table(&["Kind", "Name", "GUID", "Table", "Schema", "Live"], &rows);
    println!("({} objects)", rows.len());
}

fn print_indexes(snapshot: &MetadataSnapshot) {
    let mut rows: Vec<Vec<String>> = snapshot
        .indexes
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
        &["Metadata", "Table", "Declared", "Live", "Key", "Match"],
        &rows,
    );
    println!("({} indexes)", rows.len());
}

fn print_description(snapshot: &MetadataSnapshot, name: &str) -> Result<(), CliError> {
    let object =
        find_metadata_object(snapshot, name).map_err(|error| CliError::Data(error.to_string()))?;
    let fields =
        queryable_fields(snapshot, object).map_err(|error| CliError::Data(error.to_string()))?;
    println!(
        "{}  GUID={}  table={}  schema={}  live={}",
        object_display_name(Some(object)),
        object.guid,
        object.physical_table.as_deref().unwrap_or(""),
        yes_no(object.declared),
        yes_no(object.live),
    );

    let field_rows: Vec<Vec<String>> = fields
        .into_iter()
        .map(|field| {
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
            ]
        })
        .collect();
    println!("Attributes:");
    print_table(
        &[
            "Name",
            "Schema name",
            "Aliases",
            "Physical members",
            "Reference target",
        ],
        &field_rows,
    );

    let table = object.physical_table.as_deref().unwrap_or("");
    let index_rows: Vec<Vec<String>> = snapshot
        .indexes
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
    println!("Indexes:");
    print_table(&["Declared", "Live", "Key", "Match"], &index_rows);
    Ok(())
}

fn object_for_table<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    table: &str,
) -> Option<&'snapshot MetadataObject> {
    snapshot.objects.iter().find(|object| {
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

fn print_query_rows(compiled: &CompiledQuery, rows: &QueryRows) -> Result<(), CliError> {
    let mut rendered = Vec::with_capacity(rows.len());
    for row in rows {
        let mut values = Vec::with_capacity(compiled.columns.len());
        if row.len() != compiled.columns.len() {
            return Err(CliError::Data(format!(
                "database returned {} columns, expected {}",
                row.len(),
                compiled.columns.len()
            )));
        }
        for value in row {
            values.push(value.clone().unwrap_or_else(|| "NULL".to_owned()));
        }
        rendered.push(values);
    }
    let headers: Vec<&str> = compiled.columns.iter().map(String::as_str).collect();
    print_table(&headers, &rendered);
    println!("({} rows)", rows.len());
    Ok(())
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (index, value) in row.iter().enumerate().take(widths.len()) {
            widths[index] = widths[index].max(display_width(value));
        }
    }
    print_table_row(
        &headers
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
        &widths,
    );
    println!(
        "{}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("-+-")
    );
    for row in rows {
        print_table_row(row, &widths);
    }
}

fn print_table_row(values: &[String], widths: &[usize]) {
    let cells: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let value = values.get(index).map_or("", String::as_str);
            let value = escape_field(value);
            format!("{value:<width$}")
        })
        .collect();
    println!("{}", cells.join(" | "));
}

fn display_width(value: &str) -> usize {
    escape_field(value).chars().count()
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
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use moka::future::Cache;
    use open_sdbl::metadata::{FieldId, MetadataKind, ObjectId, StandardFieldId};
    use open_sdbl::query::{PresentationExpression, PresentationPlan};
    use rustyline::highlight::Highlighter;

    use super::{
        ConsoleHelper, PRESENTATION_POLICY_VERSION, PresentationPlanKey, completion_start,
        decode_input_line, default_presentation_template, footer_text, format_duration,
        push_unique, push_virtual_table_candidates, statement_is_complete, timing_line,
    };

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
    fn completes_commands_keywords_and_cyrillic_metadata_case_insensitively() {
        let helper = ConsoleHelper {
            candidates: vec![
                "\\refresh".to_owned(),
                "ВЫБРАТЬ".to_owned(),
                "Справочник.Договоры".to_owned(),
                "Организация.Код".to_owned(),
            ],
            source_candidates: vec!["Справочник.Договоры".to_owned()],
            known_identifiers: HashSet::new(),
        };

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
        let helper = ConsoleHelper {
            source_candidates: candidates.clone(),
            candidates,
            known_identifiers: HashSet::new(),
        };

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
    fn restricts_source_completion_to_the_qualified_metadata_hierarchy() {
        let helper = ConsoleHelper {
            candidates: vec![
                "Код".to_owned(),
                "Договоры".to_owned(),
                "_Референс42".to_owned(),
                "Организация.Код".to_owned(),
            ],
            source_candidates: vec![
                "Catalog.Contracts".to_owned(),
                "Document.Sale".to_owned(),
                "РегистрНакопления.Остатки".to_owned(),
                "РегистрНакопления.Остатки.Остатки()".to_owned(),
                "Справочник.Договоры".to_owned(),
            ],
            known_identifiers: HashSet::new(),
        };

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
    fn finds_completion_boundary_without_splitting_utf8_or_dotted_names() {
        let line = "ВЫБРАТЬ Организация.Ко";
        assert_eq!(completion_start(line, line.len()), "ВЫБРАТЬ ".len());
        assert_eq!(completion_start("\\d Спр", "\\d Спр".len()), "\\d ".len());
    }

    #[test]
    fn highlights_lexer_tokens_without_changing_display_text() {
        let helper = ConsoleHelper {
            candidates: Vec::new(),
            source_candidates: Vec::new(),
            known_identifiers: HashSet::from(["договоры".to_owned()]),
        };
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

    #[tokio::test]
    async fn presentation_cache_reuses_a_generation_and_separates_refreshes() {
        let cache = Cache::builder().max_capacity(8).build();
        let object = ObjectId::from_bytes([7; 16]);
        let calls = Arc::new(AtomicUsize::new(0));
        for generation in [0, 0, 1] {
            let calls = Arc::clone(&calls);
            let key = PresentationPlanKey {
                metadata_generation: generation,
                object,
                language: "ru",
                policy_version: PRESENTATION_POLICY_VERSION,
            };
            cache
                .get_with(key, async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    PresentationPlan {
                        object,
                        fields: Vec::new(),
                        expression: PresentationExpression::Literal(String::new()),
                    }
                })
                .await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
