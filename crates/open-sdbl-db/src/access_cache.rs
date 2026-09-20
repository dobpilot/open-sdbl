//! The session parameters of the restriction templates, read from the
//! base itself.
//!
//! The Standard Subsystems Library keeps, in the information register
//! `ПараметрыОграниченияДоступа`, a value storage holding the values its
//! templates read: the versions of the templates and the lists whose
//! reading is restricted, by access key or by field. This module reads
//! that value the way the platform does — the column carries the key of
//! the content in `binarydata` — and answers the values of those names,
//! for the caller to store beside the ones it already has.

use open_sdbl::metadata::{
    DEFAULT_OUTPUT_LIMIT, Guid, MetadataSnapshot, MsSqlMetadataQueries, ObjectId,
    PostgresMetadataQueries, decode_stored_value, entry_string, parse_stored_value_ref,
    stored_map_entries,
};
use open_sdbl::query::{
    CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, Prepared, QueryCompiler,
    QueryDiagnostic, QueryDiagnosticKind, QueryParameter, SessionParameters, TempTablesManager,
    find_metadata_object,
};

use crate::cells::Cell;
use crate::error::DbError;
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

/// The values the templates of the base read, or why they were not read.
pub struct TemplateParameters {
    /// The parameters read, as `(имя, значение)` pairs.
    pub values: Vec<(String, String)>,
    /// Why the register the configuration carries could not be read;
    /// `None` when it was read, or when the configuration has none.
    pub unread: Option<String>,
}

impl TemplateParameters {
    fn none() -> Self {
        Self {
            values: Vec::new(),
            unread: None,
        }
    }

    fn unread(reason: String) -> Self {
        Self {
            values: Vec::new(),
            unread: Some(reason),
        }
    }
}

/// Reads the values the templates of the base read.
///
/// Answers nothing when the base carries no such register — a
/// configuration without the library — the reason when the register is
/// there and the reading of it needs what the session lacks, and an
/// error only when the base answers one.
pub async fn read_template_parameters(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
) -> Result<TemplateParameters, DbError> {
    let sql = match compile_cache_query(session, snapshot, session_parameters) {
        CacheQuery::Sql(sql) => sql,
        CacheQuery::Absent => return Ok(TemplateParameters::none()),
        CacheQuery::Unread(reason) => return Ok(TemplateParameters::unread(reason)),
    };
    let rows = session.query(&sql, 1).await?;
    let Some(cell) = rows.first().and_then(|row| row.first()) else {
        return Ok(TemplateParameters::unread(
            "the register carries no row".to_owned(),
        ));
    };
    let bytes = cell_bytes(cell);
    // The column carries the value itself, or the reference to the parts
    // the platform stores apart.
    let content = match parse_stored_value_ref(&bytes) {
        None => bytes,
        Some(reference) => {
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
            content
        }
    };
    if content.is_empty() {
        return Ok(TemplateParameters::unread(
            "the stored value has no content".to_owned(),
        ));
    }
    let value = decode_stored_value(&content, DEFAULT_OUTPUT_LIMIT)
        .map_err(|error| DbError::Data(format!("ПараметрыОграниченияДоступа: {error}")))?;
    Ok(TemplateParameters {
        values: template_parameters(&value),
        unread: None,
    })
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
pub async fn read_current_user(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
    user: &Guid,
) -> Result<Option<ParameterValue>, DbError> {
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

/// What compiling the reading of the cache answered.
enum CacheQuery {
    /// The statement to run.
    Sql(String),
    /// The configuration carries no such register.
    Absent,
    /// The register is there, and the statement needs what the session
    /// lacks.
    Unread(String),
}

/// Compiles the reading of the cache.
fn compile_cache_query(
    session: &DatabaseSession,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
) -> CacheQuery {
    let options = CompileOptions::new().session(session_parameters);
    let prepared = match session.dialect() {
        DatabaseDialect::Postgres => QueryCompiler::new(snapshot, PostgresBackend)
            .prepare(CACHE_QUERY)
            .map(PreparedCache::Postgres),
        DatabaseDialect::MsSql { backend } => QueryCompiler::new(snapshot, backend)
            .prepare(CACHE_QUERY)
            .map(PreparedCache::MsSql),
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        // A configuration without the library has no such register.
        Err(error) if error.kind() == QueryDiagnosticKind::UnknownObject => {
            return CacheQuery::Absent;
        }
        Err(error) => return CacheQuery::Unread(error.to_string()),
    };
    let mut temporary = TempTablesManager::new();
    match prepared.compile(snapshot, &options, &mut temporary) {
        Ok(Some(compiled)) => CacheQuery::Sql(compiled),
        Ok(None) => CacheQuery::Absent,
        Err(error) => CacheQuery::Unread(error.to_string()),
    }
}

/// The reading of the cache prepared for the dialect of the session.
enum PreparedCache {
    Postgres(Prepared<PostgresBackend>),
    MsSql(Prepared<MsSqlBackend>),
}

impl PreparedCache {
    fn compile(
        self,
        snapshot: &MetadataSnapshot,
        options: &CompileOptions<'_>,
        temporary: &mut TempTablesManager,
    ) -> Result<Option<String>, QueryDiagnostic> {
        let compiled = match self {
            Self::Postgres(query) => query.compile_batch(snapshot, options, temporary),
            Self::MsSql(query) => query.compile_batch(snapshot, options, temporary),
        }?;
        Ok(compiled.map(|compiled| compiled.sql))
    }
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
