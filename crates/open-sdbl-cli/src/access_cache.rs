//! The session parameters of the restriction templates, read from the
//! base itself.
//!
//! The Standard Subsystems Library keeps, in the information register
//! `ПараметрыОграниченияДоступа`, a value storage holding the values its
//! templates read: the versions of the templates and the lists whose
//! reading is restricted, by access key or by field. The console reads
//! that value the way the platform does — the column carries the key of
//! the content in `binarydata` — and fills the session parameters of
//! those names, leaving alone the ones the operator typed.

use open_sdbl::metadata::{
    DEFAULT_OUTPUT_LIMIT, Guid, MetadataSnapshot, MsSqlMetadataQueries, ObjectId,
    PostgresMetadataQueries, decode_stored_value, entry_string, parse_stored_value_ref,
    stored_map_entries,
};
use open_sdbl::query::{
    CompileOptions, ParameterValue, PostgresBackend, QueryCompiler, QueryParameter,
    SessionParameters, TempTablesManager, find_metadata_object,
};

use crate::cells::Cell;
use crate::error::CliError;
use crate::session::{DatabaseDialect, DatabaseSession};

#[cfg(test)]
#[path = "tests/access_cache.rs"]
mod tests;

/// The register and the field the library keeps the values in.
const CACHE_QUERY: &str = "ВЫБРАТЬ ПЕРВЫЕ 1 ДляШаблоновВСеансахПользователей ИЗ РегистрСведений.ПараметрыОграниченияДоступа УПОРЯДОЧИТЬ ПО Версия УБЫВ";

/// The session parameter each key of the cache carries the value of.
const PARAMETERS: [(&str, &str); 5] = [
    ("ВерсииШаблонов", "ВерсииШаблоновОграниченияДоступа"),
    (
        "СпискиСОграничениемЧерезКлючиДоступаГруппДоступа",
        "СпискиСОграничениемЧерезКлючиДоступаГруппДоступа",
    ),
    (
        "СпискиСОграничениемЧерезКлючиДоступаПользователей",
        "СпискиСОграничениемЧерезКлючиДоступаПользователей",
    ),
    ("СпискиСОграничениемПоПолям", "СпискиСОграничениемПоПолям"),
    (
        "СпискиСОтключеннымОграничениемЧтения",
        "СпискиСОтключеннымОграничениемЧтения",
    ),
];

/// Reads the values the templates of the base read, as
/// `(имя параметра, значение)` pairs.
///
/// Answers an empty list when the base carries no such register — a
/// configuration without the library — and an error only when the
/// register is there and cannot be read.
pub(crate) async fn read_template_parameters(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
) -> Result<Vec<(String, String)>, CliError> {
    let Some(sql) = compile_cache_query(session, snapshot, session_parameters) else {
        return Ok(Vec::new());
    };
    let rows = session.query(&sql, 1).await?;
    let Some(cell) = rows.first().and_then(|row| row.first()) else {
        return Ok(Vec::new());
    };
    let bytes = cell_bytes(cell);
    let Some(reference) = parse_stored_value_ref(&bytes) else {
        return Ok(Vec::new());
    };
    let statement = match session.dialect() {
        DatabaseDialect::Postgres => PostgresMetadataQueries::stored_value(&reference),
        DatabaseDialect::MsSql { .. } => MsSqlMetadataQueries::stored_value(&reference),
    };
    let parts = session.query(&statement, 1).await?;
    let mut content = Vec::new();
    for row in &parts {
        if let Some(cell) = row.first() {
            content.extend_from_slice(&cell_bytes(cell));
        }
    }
    if content.is_empty() {
        return Ok(Vec::new());
    }
    let value = decode_stored_value(&content, DEFAULT_OUTPUT_LIMIT)
        .map_err(|error| CliError::Data(format!("ПараметрыОграниченияДоступа: {error}")))?;
    Ok(template_parameters(&value))
}

/// The session parameters the decoded cache carries.
pub(crate) fn template_parameters(value: &open_sdbl::metadata::Value) -> Vec<(String, String)> {
    let mut values = Vec::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        for (key, entry) in stored_map_entries(value) {
            if let Some((_, parameter)) = PARAMETERS.iter().find(|(name, _)| *name == key) {
                if let Some(text) = entry_string(entry) {
                    values.push(((*parameter).to_owned(), text));
                }
            } else {
                // `ПараметрыШаблонов` nests a map of its own.
                pending.push(entry);
            }
        }
    }
    values.sort();
    values.dedup_by(|left, right| left.0 == right.0);
    values
}

/// The catalog of the users and the attribute carrying the identifier of
/// the information-base user.
const USERS_QUERY: &str = "ВЫБРАТЬ ПЕРВЫЕ 1 Ссылка ИЗ Справочник.Пользователи ГДЕ ИдентификаторПользователяИБ = &ИдентификаторПользователяИБ";

/// Reads the element of `Справочник.Пользователи` of the information-base
/// user, for the session parameter `ТекущийПользователь`.
///
/// Answers `None` when the configuration has no such catalog or
/// attribute, or carries no element of that user.
pub(crate) async fn read_current_user(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
    user: &Guid,
) -> Result<Option<ParameterValue>, CliError> {
    let Ok(object) = find_metadata_object(snapshot, "Справочник.Пользователи")
    else {
        return Ok(None);
    };
    let parameters = [QueryParameter::new(
        "ИдентификаторПользователяИБ",
        ParameterValue::Binary(user.to_1c_bytes().to_vec()),
    )];
    let options = CompileOptions::new()
        .session(session_parameters)
        .parameters(&parameters);
    let Some(sql) = compile(session, snapshot, USERS_QUERY, &options) else {
        return Ok(None);
    };
    let rows = session.query(&sql, 1).await?;
    let Some(bytes) = rows.first().and_then(|row| row.first()).map(cell_bytes) else {
        return Ok(None);
    };
    let Ok(id) = <[u8; 16]>::try_from(bytes.as_slice()) else {
        return Ok(None);
    };
    Ok(Some(ParameterValue::Reference {
        object: ObjectId::from(&object.guid),
        id,
    }))
}

/// The bytes of a cell, whichever way the provider answers them.
fn cell_bytes(cell: &Cell) -> Vec<u8> {
    match cell {
        Cell::Bytes(bytes) => bytes.clone(),
        Cell::Text(text) => decode_hex(text).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Compiles the reading of the cache; `None` when the configuration has
/// no such register, or the statement needs what the session lacks.
fn compile_cache_query(
    session: &DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
) -> Option<String> {
    let options = CompileOptions::new().session(session_parameters);
    compile(session, snapshot, CACHE_QUERY, &options)
}

/// Compiles one statement of this module; `None` when the configuration
/// does not carry what it reads, or the session lacks what it needs.
fn compile(
    session: &DatabaseSession,
    snapshot: &MetadataSnapshot,
    statement: &str,
    options: &CompileOptions<'_>,
) -> Option<String> {
    let mut temporary = TempTablesManager::new();
    let compiled = match session.dialect() {
        DatabaseDialect::Postgres => QueryCompiler::new(snapshot, PostgresBackend)
            .prepare(statement)
            .ok()?
            .compile_batch(snapshot, options, &mut temporary),
        DatabaseDialect::MsSql { backend } => QueryCompiler::new(snapshot, backend)
            .prepare(statement)
            .ok()?
            .compile_batch(snapshot, options, &mut temporary),
    };
    Some(compiled.ok()??.sql)
}

/// Reads the `0x…` hexadecimal a provider may answer bytes as.
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}
