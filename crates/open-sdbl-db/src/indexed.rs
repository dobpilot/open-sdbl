//! Адресное приобретение ресурсов без полного декодирования метаданных.

use open_sdbl::metadata::{ContentKey, StorageLayout};

use crate::error::DbError;
use crate::limits::Limits;
use crate::pipeline::{ConfigResource, ConfigSelector, ExtensionCatalogRow, MetadataSource};

/// Начальные ресурсы конфигурации без декодированного каталога метаданных.
///
/// ```no_run
/// use open_sdbl_db::{BlockingDatabaseSession, ConfigSelector, DbError};
/// fn read(session: &mut BlockingDatabaseSession, selector: &mut dyn ConfigSelector)
///     -> Result<(), DbError>
/// {
///     let bootstrap = session.configuration_bootstrap(selector)?;
///     let first = session.configuration_resources(&["first.0", "second.0"])?;
///     let next = session.configuration_resources(&["third.0"])?;
///     let _ = (bootstrap, first, next);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationBootstrap {
    /// Обнаруженная схема физического хранения, кэшируемая сеансом.
    pub layout: StorageLayout,
    /// Сжатый ресурс `root`.
    pub root: ConfigResource,
    /// Сжатый описатель конфигурации, выбранный по `root`.
    pub descriptor: ConfigResource,
    /// Все строки каталога расширений без чтения `ConfigCAS`.
    /// Ключ корня извлекается из `info` функцией `open_sdbl::metadata::extension_root_key`.
    pub extensions: Vec<ExtensionCatalogRow>,
}

pub(crate) fn names(names: &[&str], limits: Limits) -> Result<Vec<String>, DbError> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in names {
        let name = name.trim_end();
        if name.is_empty() || name.contains('\0') {
            return Err(DbError::Data("invalid empty or NUL resource name".into()));
        }
        if seen.insert(name) {
            if result.len() >= limits.config_resource_limit {
                return Err(DbError::Data(
                    "indexed resource count exceeds retention limit".into(),
                ));
            }
            result.push(name.to_owned());
        }
    }
    Ok(result)
}

pub(crate) fn keys(keys: &[ContentKey]) -> Vec<String> {
    keys.iter().map(ContentKey::as_hex).collect()
}

pub(crate) fn sql(base: &str, names: &[String], mssql: bool) -> String {
    let literals = names
        .iter()
        .map(|name| {
            let escaped = name.replace('\'', "''");
            if mssql {
                format!("N'{escaped}'")
            } else {
                format!("E'{}'", escaped.replace('\\', "\\\\"))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let column = if mssql { "[FileName]" } else { "filename" };
    base.replacen(
        " ORDER BY ",
        &format!(" WHERE {column} IN ({literals}) ORDER BY "),
        1,
    )
}

pub(crate) fn totals_sql(base: &str, requested: &[String], mssql: bool) -> String {
    let addressed = sql(base, requested, mssql);
    let (_, from) = addressed
        .split_once(" FROM ")
        .expect("provider resource SELECT");
    let (from, _) = from
        .split_once(" ORDER BY ")
        .expect("provider resource ordering");
    if mssql {
        format!(
            "SELECT COUNT_BIG(DISTINCT [FileName]), COALESCE(SUM(CONVERT(bigint, DATALENGTH([BinaryData]))), CONVERT(bigint, 0)) FROM {from}"
        )
    } else {
        format!(
            "SELECT COUNT(DISTINCT filename), COALESCE(SUM(octet_length(binarydata)), 0)::bigint FROM {from}"
        )
    }
}

pub(crate) fn ordered(
    requested: &[String],
    resources: Vec<ConfigResource>,
) -> Result<Vec<ConfigResource>, DbError> {
    let mut resources = resources
        .into_iter()
        .map(|resource| (resource.file_name.clone(), resource))
        .collect::<std::collections::HashMap<_, _>>();
    let result = requested
        .iter()
        .map(|name| {
            resources.remove(name).ok_or_else(|| {
                DbError::Data(format!(
                    "requested configuration resource {name:?} is missing"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !resources.is_empty() {
        return Err(DbError::Data(
            "addressed query returned an unrequested resource".into(),
        ));
    }
    Ok(result)
}

/// Ограничивает вход до сборки частей: большой единичный ресурс не может
/// обойти общий потолок, пока `assemble_parts` ещё не выдал значение.
pub(crate) fn bound_part(
    part: Result<crate::pipeline::ResourcePart, DbError>,
    held: &mut usize,
    limits: Limits,
) -> Result<crate::pipeline::ResourcePart, DbError> {
    let part = part?;
    *held = held
        .checked_add(part.2.len())
        .filter(|bytes| *bytes <= limits.config_retained_byte_limit)
        .ok_or_else(|| DbError::Data("indexed resource bytes exceed retention limit".into()))?;
    Ok(part)
}

pub(crate) async fn bootstrap(
    source: &mut impl MetadataSource,
    known: Option<StorageLayout>,
    selector: &mut dyn ConfigSelector,
    limits: Limits,
) -> Result<ConfigurationBootstrap, DbError> {
    let result = async {
        source.begin_readonly().await?;
        let layout = match known {
            Some(layout) => layout,
            None => source.detect_layout().await?,
        };
        if limits.config_resource_limit < 2 {
            return Err(DbError::Data(
                "bootstrap resources exceed retention limits".into(),
            ));
        }
        let root = source
            .read_indexed_resources(&layout, &["root".into()], false, limits)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| DbError::Data("Config root is missing".into()))?;
        let descriptor_name = selector
            .root_resource_name(&root)?
            .parse::<open_sdbl::metadata::Guid>()?
            .to_string();
        let mut remaining = limits;
        remaining.config_retained_byte_limit = limits
            .config_retained_byte_limit
            .checked_sub(root.compressed.len())
            .ok_or_else(|| DbError::Data("bootstrap resources exceed retention limits".into()))?;
        remaining.config_resource_limit -= 1;
        let descriptor = source
            .read_indexed_resources(&layout, &[descriptor_name], false, remaining)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| DbError::Data("configuration descriptor is missing".into()))?;
        let bytes = root
            .compressed
            .len()
            .checked_add(descriptor.compressed.len())
            .ok_or_else(|| DbError::Data("bootstrap resource byte count overflow".into()))?;
        if limits.config_resource_limit < 2 || bytes > limits.config_retained_byte_limit {
            return Err(DbError::Data(
                "bootstrap resources exceed retention limits".into(),
            ));
        }
        selector.configure(&descriptor)?;
        let extensions = source.read_extension_catalog(&layout).await?;
        source.commit_readonly().await?;
        Ok(ConfigurationBootstrap {
            layout,
            root,
            descriptor,
            extensions,
        })
    }
    .await;
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

pub(crate) async fn resources(
    source: &mut impl MetadataSource,
    known: Option<StorageLayout>,
    requested: &[String],
    extension: bool,
    limits: Limits,
) -> Result<(StorageLayout, Vec<ConfigResource>), DbError> {
    let result = async {
        source.begin_readonly().await?;
        let layout = match known {
            Some(layout) => layout,
            None => source.detect_layout().await?,
        };
        let resources = source
            .read_indexed_resources(&layout, requested, extension, limits)
            .await?;
        source.commit_readonly().await?;
        Ok((layout, resources))
    }
    .await;
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

#[cfg(test)]
#[path = "tests/indexed.rs"]
mod tests;
