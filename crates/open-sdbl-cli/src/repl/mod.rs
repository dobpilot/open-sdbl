//! The console: read a statement, run it, print the result.
//!
//! Everything the console needs lives in a module of its own — this one
//! carries the loop that ties them together.

use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{PostgresBackend, QueryCompiler, TempTablesManager};
use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use tokio::io::BufReader;

use crate::access::{AccessStore, apply_access_command, parse_access_command, user_restrictions};
use crate::error::CliError;
use crate::output::escape_field;
use crate::params::{ParameterStore, apply_parameter_command, parse_parameter_command};
use crate::restrict::{RestrictionStore, apply_restriction_command, parse_restriction_command};
use crate::session::{DatabaseDialect, DatabaseSession};

mod completion;
mod describe;
mod meta;
mod prepare;
mod presentation;
mod render;
mod terminal;

use completion::*;
use describe::*;
use meta::*;
use prepare::*;
use presentation::*;
use render::*;
use terminal::*;

#[cfg(test)]
#[path = "../tests/repl_loop.rs"]
mod tests;

pub(super) const CONSOLE_HELP: &str = "Commands:
  \\dt                 list resolved metadata tables
  \\di                 list declared and live indexes
  \\d <metadata-name>  describe attributes and indexes
  \\refresh            reload DBNames, Config, SchemaStorage, and catalogs
  \\set <name> <lit>   store a query parameter (&name) from an SDBL literal
  \\params             list stored parameters
  \\unset <name>       remove a stored parameter
  \\session [<name> [=] <lit> | clear]
                      store, list, or clear session parameters seen by
                      every query and access restriction
  \\restrict [<Вид>.<Объект>[.<ТабЧасть>] <condition> | clear]
                      store, list, or clear access restrictions applied to
                      ВЫБРАТЬ РАЗРЕШЕННЫЕ (SDBL condition over the table)
  \\tables             list temporary tables placed in this session
  \\users              list the users of the base (v8users)
  \\user <name>        show one user with the names of its roles
  \\roles [<text>]     list the roles of the configuration
  \\role <name> [<Вид>.<Объект>]
                      show what a role grants, or its rights and
                      restriction texts on one object
  \\rls <Вид>.<Объект> [<right>]
                      show the restriction texts and the expanded access
                      of the current user (or every role) to the object
  \\as <user> | clear  run ВЫБРАТЬ РАЗРЕШЕННЫЕ as that user: the roles'
                      restrictions apply where \\restrict sets none
  \\help               show this help
  \\q                  quit

Enter a supported 1C SELECT query and terminate it with a semicolon.
Statements placing temporary tables (ПОМЕСТИТЬ, ДОБАВИТЬ, УНИЧТОЖИТЬ) keep
them for the rest of the session; \\refresh forgets them.
";

#[cfg(any(target_os = "linux", test))]
pub(super) const COMMAND_HINT: &str =
    "\\dt tables  \\di indexes  \\d <name> describe  \\refresh reload  \\help  \\q quit";

pub(super) type ConsoleEditor = Editor<ConsoleHelper, DefaultHistory>;

pub(super) const COMPLETION_KEYWORDS: &[&str] = &[
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
    "СРЕДНЕЕ",
    "AVG",
    "ССЫЛКА",
    "REFS",
    "МЕЖДУ",
    "BETWEEN",
    "ПОДСТРОКА",
    "SUBSTRING",
    "ДЛИНАСТРОКИ",
    "STRINGLENGTH",
    "СОКРЛП",
    "СОКРЛ",
    "СОКРП",
    "ВРЕГ",
    "НРЕГ",
    "СТРНАЙТИ",
    "СТРЗАМЕНИТЬ",
    "ОКР",
    "ЦЕЛ",
    "ТИП",
    "TYPE",
    "ТИПЗНАЧЕНИЯ",
    "VALUETYPE",
    "НЕОПРЕДЕЛЕНО",
    "UNDEFINED",
    "ИТОГИ",
    "TOTALS",
    "ОБЩИЕ",
    "OVERALL",
    "ИЕРАРХИЯ",
    "HIERARCHY",
    "ТОЛЬКО",
    "ONLY",
    "ПЕРИОДАМИ",
    "PERIODS",
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
    "КОНЕЦПЕРИОДА",
    "ENDOFPERIOD",
    "ДОБАВИТЬКДАТЕ",
    "DATEADD",
    "РАЗНОСТЬДАТ",
    "DATEDIFF",
    "ГОД",
    "YEAR",
    "КВАРТАЛ",
    "QUARTER",
    "МЕСЯЦ",
    "MONTH",
    "ДЕНЬГОДА",
    "DAYOFYEAR",
    "ДЕНЬ",
    "DAY",
    "НЕДЕЛЯ",
    "WEEK",
    "ДЕНЬНЕДЕЛИ",
    "WEEKDAY",
    "ЧАС",
    "HOUR",
    "МИНУТА",
    "MINUTE",
    "СЕКУНДА",
    "SECOND",
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

pub(super) const PRESENTATION_POLICY_VERSION: u32 = 2;
pub(super) const MAX_INPUT_LINE_BYTES: usize = 1024 * 1024;
pub(super) const UNRESOLVED_REFERENCE: &str = "<unresolved reference>";

pub(super) fn add_history(editor: &mut Option<ConsoleEditor>, entry: &str) {
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
    let mut session_parameters = ParameterStore::new();
    let mut restrictions = RestrictionStore::new();
    let mut access = AccessStore::new(&snapshot);
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
                match apply_parameter_command(
                    &mut parameters,
                    &mut session_parameters,
                    command,
                    &snapshot,
                ) {
                    Ok(text) => {
                        output
                            .write_all(text.as_bytes())
                            .map_err(CliError::standard_output)?;
                        if let Some(helper) = editor.as_mut().and_then(Editor::helper_mut) {
                            helper
                                .set_parameters(parameter_names(&parameters, &session_parameters));
                        }
                    }
                    Err(error) => eprintln!("error: {}", escape_field(&error.to_string())),
                }
                continue;
            }
            if let Some(command) = parse_restriction_command(line.trim()) {
                match apply_restriction_command(&mut restrictions, command, &snapshot) {
                    Ok(text) => output
                        .write_all(text.as_bytes())
                        .map_err(CliError::standard_output)?,
                    Err(error) => eprintln!("error: {}", escape_field(&error.to_string())),
                }
                continue;
            }
            if let Some(command) = parse_access_command(line.trim()) {
                let session_values = session_parameters.session_parameters();
                if let Err(error) = apply_access_command(
                    &mut access,
                    command,
                    session,
                    &snapshot,
                    &session_values,
                    output,
                )
                .await
                {
                    eprintln!("error: {}", escape_field(&error.to_string()));
                    ensure_session_remains_usable(session.is_dead())?;
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
                    access.reset(&snapshot);
                    // Definitions hold SQL generated against the old
                    // snapshot, so they cannot survive a reload.
                    if !temporary_tables.is_empty() {
                        temporary_tables.clear();
                        writeln!(output, "Temporary tables cleared.")
                            .map_err(CliError::standard_output)?;
                    }
                    if let Some(helper) = editor.as_mut().and_then(Editor::helper_mut) {
                        *helper = ConsoleHelper::from_snapshot(&snapshot);
                        helper.set_parameters(parameter_names(&parameters, &session_parameters));
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
                let session = session_parameters.session_parameters();
                let mut applied = restrictions.for_request(prepared.restriction_request());
                match user_restrictions(
                    &access,
                    &snapshot,
                    prepared.restriction_request(),
                    &applied,
                    &session,
                ) {
                    Ok(derived) => applied.extend(derived),
                    Err(error) => {
                        eprintln!("error: {}", escape_field(&error.to_string()));
                        statement.clear();
                        continue;
                    }
                }
                prepared.compile_batch(
                    &snapshot,
                    &plans,
                    &values,
                    &session,
                    &applied,
                    &mut temporary_tables,
                )
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
                        print_query_rows(output, &snapshot, &compiled, &rows)
                            .map_err(CliError::standard_output)?;
                        // A projected tabular section is answered by a
                        // statement of its own, which the console runs and
                        // prints under the section's name.
                        for nested in &compiled.nested {
                            writeln!(output, "-- {} --", escape_field(&nested.label))
                                .map_err(CliError::standard_output)?;
                            match session.query(&nested.sql, nested.columns.len()).await {
                                Ok(nested_rows) => print_result_table(
                                    output,
                                    &snapshot,
                                    &nested.columns,
                                    &[],
                                    &nested_rows,
                                )
                                .map_err(CliError::standard_output)?,
                                Err(error) => {
                                    eprintln!("error: {}", escape_field(&error.to_string()));
                                    ensure_session_remains_usable(session.is_dead())?;
                                }
                            }
                        }
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
