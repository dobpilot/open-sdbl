//! Физическое обнаружение отдельно от гидратации логических объектов.

use crate::pipeline::{ConfigDecodeLimits, finish_config_metadata};
use crate::{ConfigResource, DbError, Limits, MetadataSource};
use futures_util::{Stream, StreamExt};
use open_sdbl::metadata::{
    DbNames, ExtensionMetadata, LiveTable, ResolvedMetadata, RuntimeMetadataFacts, SchemaStorage,
    StorageLayout, parse_config_resource_bounded, resolve_selected_metadata,
    resolve_selected_runtime_metadata,
};

/// Переиспользуемые физические индексы без подробных объектов конфигурации.
#[derive(Debug, Clone)]
pub struct ConfigurationMetadataInputs {
    /// Схема физического хранения принадлежащей сеансу базы.
    pub layout: StorageLayout,
    /// Авторитетное соответствие GUID физическим номерам, включая разделители.
    pub db_names: DbNames,
    /// Декларации физической схемы, прочитанные один раз.
    pub schema: SchemaStorage,
    /// Реально наблюдавшиеся таблицы, столбцы и индексы.
    pub live_tables: Vec<LiveTable>,
}

/// Строит снимок только из переданных ресурсов и явных структур расширений.
/// Не выполняет SQL и не приобретает зависимости. Связанные описатели,
/// включая настройки разделителей, передаёт вызывающая сторона.
/// Распаковка ограничена обычными бюджетами декодера конфигурации.
///
/// # Errors
/// Возвращает ошибку повреждённого ресурса, превышения бюджета распаковки
/// либо неподдержанных настроек разделителя. Отсутствующие описатели разделителя
/// и привязанных параметров возвращаются как `ConfigurationDescriptorsMissing`;
/// вызывающая сторона приобретает указанные GUID и повторяет разрешение.
pub fn resolve_configuration_resources(
    inputs: &ConfigurationMetadataInputs,
    resources: &[ConfigResource],
    extensions: Vec<ExtensionMetadata>,
) -> Result<ResolvedMetadata, DbError> {
    let limits = ConfigDecodeLimits::default();
    let mut decoded = 0usize;
    let mut parsed_resources = Vec::new();
    for resource in resources {
        let remaining = limits.total_bytes.saturating_sub(decoded);
        let parsed = parse_config_resource_bounded(
            &resource.file_name,
            &resource.compressed,
            limits.resource_bytes.min(remaining),
        )?;
        decoded = decoded
            .checked_add(parsed.decoded_bytes)
            .filter(|bytes| *bytes <= limits.total_bytes)
            .ok_or_else(|| {
                DbError::Data("explicit Config resources exceed decoded byte limit".into())
            })?;
        parsed_resources.push(parsed);
    }
    let resource_guids = resources
        .iter()
        .filter_map(|resource| resource.file_name.parse().ok())
        .collect();
    resolve_parsed_configuration(inputs, parsed_resources, &resource_guids, extensions)
}

pub(crate) fn resolve_parsed_configuration(
    inputs: &ConfigurationMetadataInputs,
    resources: impl IntoIterator<Item = open_sdbl::metadata::ParsedConfigResource>,
    resource_guids: &std::collections::HashSet<open_sdbl::metadata::Guid>,
    extensions: Vec<ExtensionMetadata>,
) -> Result<ResolvedMetadata, DbError> {
    let (descriptors, predefined, criteria, roles) = collect_parsed_metadata(resources);
    let mut resolved = resolve_selected_metadata(
        &inputs.db_names,
        descriptors,
        predefined,
        extensions,
        &inputs.schema,
        &inputs.live_tables,
    );
    validate_separator_descriptors(resolved.snapshot.runtime_facts(), resource_guids)?;
    resolved.snapshot.attach_criteria(criteria);
    resolved.snapshot.attach_roles(roles);
    Ok(resolved)
}

pub(crate) fn resolve_parsed_runtime_configuration(
    inputs: &ConfigurationMetadataInputs,
    resources: impl IntoIterator<Item = open_sdbl::metadata::ParsedConfigResource>,
    resource_guids: &std::collections::HashSet<open_sdbl::metadata::Guid>,
    extensions: Vec<ExtensionMetadata>,
) -> Result<RuntimeMetadataFacts, DbError> {
    let (descriptors, predefined, criteria, roles) = collect_parsed_metadata(resources);
    let mut facts = resolve_selected_runtime_metadata(
        &inputs.db_names,
        descriptors,
        predefined,
        extensions,
        &inputs.schema,
        &inputs.live_tables,
    );
    validate_separator_descriptors(&facts, resource_guids)?;
    facts.attach_criteria(criteria);
    facts.attach_roles(roles);
    Ok(facts)
}

fn collect_parsed_metadata(
    resources: impl IntoIterator<Item = open_sdbl::metadata::ParsedConfigResource>,
) -> crate::pipeline::ConfigMetadata {
    let mut descriptors = Vec::new();
    let mut predefined = Vec::new();
    let mut criteria = Vec::new();
    let mut roles = Vec::new();
    let mut points = Vec::new();
    for mut parsed in resources {
        descriptors.append(&mut parsed.descriptors);
        predefined.append(&mut parsed.predefined_values);
        criteria.extend(parsed.criterion);
        roles.append(&mut parsed.roles);
        points.extend(parsed.route_points);
    }
    finish_config_metadata(descriptors, predefined, criteria, roles, points)
}

pub(crate) fn validate_separator_descriptors(
    facts: &RuntimeMetadataFacts,
    resource_guids: &std::collections::HashSet<open_sdbl::metadata::Guid>,
) -> Result<(), DbError> {
    let mut missing = Vec::new();
    for separator in facts.separators() {
        let descriptor = facts
            .descriptors()
            .iter()
            .find(|descriptor| descriptor.object_guid == separator.guid);
        let Some(descriptor) = descriptor else {
            missing.push(separator.guid.clone());
            continue;
        };
        let Some(settings) = &descriptor.separation else {
            return Err(DbError::Data(format!(
                "separator {} has unsupported settings",
                separator.guid
            )));
        };
        for guid in [&settings.value_parameter, &settings.use_parameter]
            .into_iter()
            .flatten()
        {
            if !facts
                .descriptors()
                .iter()
                .any(|descriptor| &descriptor.object_guid == guid)
            {
                missing.push(guid.clone());
            }
        }
    }
    missing.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    missing.dedup();
    if let Some(guid) = missing.iter().find(|guid| resource_guids.contains(*guid)) {
        return Err(DbError::Data(format!(
            "resource {guid} has no supported own descriptor"
        )));
    }
    if !missing.is_empty() {
        return Err(DbError::ConfigurationDescriptorsMissing { objects: missing });
    }
    Ok(())
}

pub(crate) async fn acquire_inputs(
    source: &mut impl MetadataSource,
    known: Option<StorageLayout>,
) -> Result<ConfigurationMetadataInputs, DbError> {
    let read = async {
        source.begin_readonly().await?;
        let layout = match known {
            Some(layout) => layout,
            None => source.detect_layout().await?,
        };
        layout.require_schema_storage()?;
        let db_names = source.read_db_names(&layout).await?;
        let schema = source.read_schema().await?;
        let live_tables = source.read_live_tables().await?;
        source.commit_readonly().await?;
        Ok(ConfigurationMetadataInputs {
            layout,
            db_names,
            schema,
            live_tables,
        })
    }
    .await;
    match read {
        Ok(value) => Ok(value),
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

pub(crate) async fn acquire_names(
    source: &mut impl MetadataSource,
    known: Option<StorageLayout>,
) -> Result<(StorageLayout, Vec<String>), DbError> {
    let read = async {
        source.begin_readonly().await?;
        let layout = match known {
            Some(layout) => layout,
            None => source.detect_layout().await?,
        };
        let names = source.read_configuration_resource_names(&layout).await?;
        source.commit_readonly().await?;
        Ok((layout, names))
    }
    .await;
    match read {
        Ok(value) => Ok(value),
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

pub(crate) async fn acquire_extensions(
    source: &mut impl MetadataSource,
    known: Option<StorageLayout>,
) -> Result<(StorageLayout, Vec<ExtensionMetadata>), DbError> {
    let read = async {
        source.begin_readonly().await?;
        let layout = match known {
            Some(layout) => layout,
            None => source.detect_layout().await?,
        };
        let blobs = source.read_bounded_extension_restructures(&layout).await?;
        let mut extensions = Vec::new();
        for blob in blobs {
            let decoded = open_sdbl::metadata::parse_extension_restructure_bounded(
                &blob,
                ConfigDecodeLimits::default().resource_bytes,
            )?;
            extensions.push(open_sdbl::metadata::extension_metadata_from_restructure(
                "configuration extension",
                decoded,
            ));
        }
        source.commit_readonly().await?;
        Ok((layout, extensions))
    }
    .await;
    match read {
        Ok(value) => Ok(value),
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

pub(crate) async fn collect_restructures(
    stream: impl Stream<Item = Result<Vec<u8>, DbError>>,
    limits: Limits,
) -> Result<Vec<Vec<u8>>, DbError> {
    let mut stream = std::pin::pin!(stream);
    let mut blobs = Vec::new();
    let mut bytes = 0usize;
    while let Some(blob) = stream.next().await {
        let blob = blob?;
        bytes = bytes
            .checked_add(blob.len())
            .filter(|size| *size <= limits.config_retained_byte_limit)
            .ok_or_else(|| {
                DbError::Data("extension restructure bytes exceed retention limit".into())
            })?;
        if blobs.len() >= limits.config_resource_limit {
            return Err(DbError::Data(
                "extension restructure count exceeds retention limit".into(),
            ));
        }
        blobs.push(blob);
    }
    Ok(blobs)
}

pub(crate) async fn collect_names(
    stream: impl Stream<Item = Result<String, DbError>>,
    limits: Limits,
) -> Result<Vec<String>, DbError> {
    let mut stream = std::pin::pin!(stream);
    let mut names = Vec::new();
    let mut bytes = 0usize;
    while let Some(name) = stream.next().await {
        let name = name?;
        bytes = bytes
            .checked_add(name.len())
            .filter(|bytes| *bytes <= limits.config_retained_byte_limit)
            .ok_or_else(|| DbError::Data("Config filename bytes exceed retention limit".into()))?;
        if names.len() >= limits.config_resource_limit {
            return Err(DbError::Data(
                "Config filename count exceeds retention limit".into(),
            ));
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

pub(crate) fn names_sql(mssql: bool) -> &'static str {
    if mssql {
        "SELECT DISTINCT RTRIM(CONVERT(nvarchar(128), [FileName])) AS name FROM [dbo].[Config] ORDER BY name"
    } else {
        "SELECT DISTINCT rtrim(filename::text) AS name FROM config ORDER BY name"
    }
}

#[cfg(test)]
#[path = "tests/metadata_inputs.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/extension_metadata_discovery.rs"]
mod extension_metadata_tests;
