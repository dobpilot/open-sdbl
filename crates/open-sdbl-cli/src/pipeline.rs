use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use open_sdbl::metadata::{
    ExtensionMetadata, LiveColumn, LiveIndex, LiveTable, MetadataErrorKind, MetadataSnapshot,
    StorageLayout, extension_metadata_from_restructure, parse_config_resource_bounded,
    parse_extension_restructure, resolve_metadata_with_predefined_values_and_extensions,
};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::error::CliError;
use crate::limits::QUERY_TIMEOUT;
use crate::output::print_resolution_report;
use crate::progress::MetadataProgress;

const CONFIG_MAX_RESOURCE_DECODED_BYTES: usize = 32 * 1024 * 1024;
const CONFIG_MAX_BATCH_DECODED_BYTES: usize = 64 * 1024 * 1024;
const CONFIG_MAX_TOTAL_DECODED_BYTES: usize = 512 * 1024 * 1024;
const CONFIG_MAX_IN_FLIGHT_DECODED_BYTES: usize = 256 * 1024 * 1024;

pub(crate) fn decode_catalog_values(rows: Vec<[String; 5]>) -> Result<Vec<LiveTable>, CliError> {
    let mut tables = BTreeMap::<String, LiveTable>::new();
    for row in rows {
        let [tag, table_name, value, detail, columns] = row;
        let table = tables
            .entry(table_name.clone())
            .or_insert_with(|| LiveTable {
                name: table_name,
                columns: Vec::new(),
                indexes: Vec::new(),
            });
        match tag.as_str() {
            "T" => {}
            "C" => table.columns.push(LiveColumn {
                name: value,
                data_type: detail,
            }),
            "I" => table.indexes.push(LiveIndex {
                name: value,
                unique: detail == "true" || detail == "t",
                columns: columns
                    .split(',')
                    .filter(|column| !column.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }),
            _ => {
                return Err(CliError::Data(format!(
                    "unknown database catalog row tag {tag:?}"
                )));
            }
        }
    }
    Ok(tables.into_values().collect())
}

/// Decodes `_ExtensionsRestruct._restructData` blobs into extension metadata.
///
/// One `ExtensionMetadata` is produced per blob that yields recognized fields
/// or anomalies. A blob that fails to decode is skipped with a stderr warning
/// rather than aborting acquisition; a poisoned extension resource must not
/// prevent reading the base configuration.
fn decode_extension_restructures(blobs: Vec<Vec<u8>>) -> Vec<ExtensionMetadata> {
    let mut extensions = Vec::new();
    for blob in blobs {
        match parse_extension_restructure(&blob) {
            Ok(restructure) => {
                if !restructure.fields.is_empty() || !restructure.anomalies.is_empty() {
                    extensions.push(extension_metadata_from_restructure(
                        "configuration extension",
                        restructure,
                    ));
                }
            }
            Err(error) => {
                eprintln!("warning: skipping malformed extension restructure: {error}");
            }
        }
    }
    extensions
}

pub(crate) type ConfigMetadata = (
    Vec<open_sdbl::metadata::ConfigDescriptor>,
    Vec<open_sdbl::metadata::ConfigPredefinedValue>,
    Vec<open_sdbl::metadata::ConfigCriterion>,
    Vec<open_sdbl::metadata::Guid>,
);

pub(crate) trait MetadataSource {
    async fn begin_readonly(&mut self) -> Result<(), CliError>;
    /// Probes the catalog for `PartNo` columns and extension tables; runs
    /// inside the read-only transaction and never fails on a legacy base.
    async fn detect_layout(&mut self) -> Result<StorageLayout, CliError>;
    async fn read_db_names(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<open_sdbl::metadata::DbNames, CliError>;
    async fn read_config(
        &mut self,
        layout: &StorageLayout,
        progress: &mut MetadataProgress,
    ) -> Result<ConfigMetadata, CliError>;
    async fn read_extension_resources(
        &mut self,
        _layout: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, CliError> {
        Ok(Vec::new())
    }
    async fn read_extensions(
        &mut self,
        _resources: Vec<ConfigResource>,
    ) -> Result<Vec<ExtensionMetadata>, CliError> {
        Ok(Vec::new())
    }
    async fn read_extension_restructures(
        &mut self,
        _layout: &StorageLayout,
    ) -> Result<Vec<Vec<u8>>, CliError> {
        Ok(Vec::new())
    }
    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, CliError>;
    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, CliError>;
    async fn commit_readonly(&mut self) -> Result<(), CliError>;
    async fn rollback_readonly(&mut self, original: CliError) -> CliError;
}

/// Reads the metadata of a base and answers the snapshot with the storage
/// layout it was read from, which later reads of the same base reuse.
pub(crate) async fn acquire_metadata(
    source: &mut impl MetadataSource,
) -> Result<(MetadataSnapshot, StorageLayout), CliError> {
    let result = async {
        let mut progress = MetadataProgress::new();
        progress.phase("transaction");
        source.begin_readonly().await?;

        progress.phase("layout");
        let layout = source.detect_layout().await?;
        layout.require_schema_storage()?;

        progress.phase(if layout.config_parts {
            "DBNames"
        } else {
            "DBNames (legacy layout)"
        });
        let db_names = source.read_db_names(&layout).await?;
        let (descriptors, predefined_values, criteria, roles) =
            source.read_config(&layout, &mut progress).await?;

        progress.phase("extensions");
        let extension_resources = source.read_extension_resources(&layout).await?;
        let mut extensions = source.read_extensions(extension_resources).await?;
        let restructure_blobs = source.read_extension_restructures(&layout).await?;
        if !restructure_blobs.is_empty() {
            let decoded = run_metadata_blocking("extension restructure", move || {
                Ok(decode_extension_restructures(restructure_blobs))
            })
            .await?;
            extensions.extend(decoded);
        }

        progress.phase("SchemaStorage");
        let schema = source.read_schema().await?;

        progress.phase("catalog");
        let live_tables = source.read_live_tables().await?;

        progress.phase("resolve");
        let resolved = run_metadata_blocking("metadata resolution", move || {
            let mut resolved = resolve_metadata_with_predefined_values_and_extensions(
                db_names,
                descriptors,
                predefined_values,
                extensions,
                schema,
                live_tables,
            );
            resolved.snapshot.attach_criteria(criteria);
            resolved.snapshot.attach_roles(roles);
            Ok(resolved)
        })
        .await?;
        progress.finish();
        print_resolution_report(&resolved.report);
        Ok((resolved.snapshot, layout))
    }
    .await;

    match result {
        Ok(acquired) => {
            source.commit_readonly().await?;
            Ok(acquired)
        }
        Err(error) => Err(source.rollback_readonly(error).await),
    }
}

pub(crate) struct ConfigResource {
    pub(crate) file_name: String,
    pub(crate) compressed: Vec<u8>,
}

/// One `(name, part, data)` row of a file table, as every layout variant
/// returns it; legacy bases report part zero for their single row.
pub(crate) type ResourcePart = (String, i32, Vec<u8>);

/// Groups rows ordered by `(name, part)` into whole resources: consecutive
/// parts of one name are concatenated in order, and the resource is emitted
/// when the name changes or the stream ends. Parts must run `0, 1, 2, …`;
/// anything else is a data error naming the resource.
pub(crate) fn assemble_parts<S>(rows: S) -> impl Stream<Item = Result<ConfigResource, CliError>>
where
    S: Stream<Item = Result<ResourcePart, CliError>>,
{
    struct State<S> {
        rows: std::pin::Pin<Box<S>>,
        current: Option<(String, i32, Vec<u8>)>,
        exhausted: bool,
    }

    futures_util::stream::try_unfold(
        State {
            rows: Box::pin(rows),
            current: None,
            exhausted: false,
        },
        |mut state| async move {
            loop {
                if state.exhausted {
                    return Ok(None);
                }
                let Some(row) = state.rows.next().await else {
                    state.exhausted = true;
                    let finished =
                        state
                            .current
                            .take()
                            .map(|(file_name, _, compressed)| ConfigResource {
                                file_name,
                                compressed,
                            });
                    return Ok(finished.map(|resource| (resource, state)));
                };
                let (file_name, part, data) = row?;
                match &mut state.current {
                    Some((current_name, next_part, buffer)) if *current_name == file_name => {
                        check_part_sequence(&file_name, part, *next_part)?;
                        buffer.extend_from_slice(&data);
                        *next_part += 1;
                    }
                    _ => {
                        check_part_sequence(&file_name, part, 0)?;
                        let finished = state.current.replace((file_name, 1, data));
                        if let Some((file_name, _, compressed)) = finished {
                            return Ok(Some((
                                ConfigResource {
                                    file_name,
                                    compressed,
                                },
                                state,
                            )));
                        }
                    }
                }
            }
        },
    )
}

/// Concatenates the parts of one resource read eagerly, such as DBNames.
pub(crate) fn assemble_single_resource(
    name: &str,
    parts: Vec<(i32, Vec<u8>)>,
) -> Result<Vec<u8>, CliError> {
    if parts.is_empty() {
        return Err(CliError::Data(format!("{name} resource is missing")));
    }
    let mut assembled = Vec::with_capacity(parts.iter().map(|(_, data)| data.len()).sum());
    for (expected, (part, data)) in (0_i32..).zip(parts) {
        check_part_sequence(name, part, expected)?;
        assembled.extend_from_slice(&data);
    }
    Ok(assembled)
}

fn check_part_sequence(name: &str, part: i32, expected: i32) -> Result<(), CliError> {
    if part == expected {
        Ok(())
    } else {
        Err(CliError::Data(format!(
            "resource {name:?} part {part} arrived where part {expected} was expected"
        )))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ConfigDecodeLimits {
    pub(crate) resource_bytes: usize,
    pub(crate) batch_bytes: usize,
    pub(crate) total_bytes: usize,
    pub(crate) in_flight_bytes: usize,
}

impl Default for ConfigDecodeLimits {
    fn default() -> Self {
        Self {
            resource_bytes: CONFIG_MAX_RESOURCE_DECODED_BYTES,
            batch_bytes: CONFIG_MAX_BATCH_DECODED_BYTES,
            total_bytes: CONFIG_MAX_TOTAL_DECODED_BYTES,
            in_flight_bytes: CONFIG_MAX_IN_FLIGHT_DECODED_BYTES,
        }
    }
}

struct DecodedConfigBatch {
    resource_count: usize,
    compressed_bytes: usize,
    decoded_bytes: usize,
    descriptors: Vec<open_sdbl::metadata::ConfigDescriptor>,
    predefined_values: Vec<open_sdbl::metadata::ConfigPredefinedValue>,
    criteria: Vec<open_sdbl::metadata::ConfigCriterion>,
    roles: Vec<open_sdbl::metadata::Guid>,
    /// Resources the decoder could not read, with the reason.
    skipped: Vec<String>,
}

pub(crate) async fn decode_config_stream<S>(
    resources: S,
    batch_size: usize,
    pipeline_depth: usize,
    limits: ConfigDecodeLimits,
    progress: &mut MetadataProgress,
) -> Result<ConfigMetadata, CliError>
where
    S: Stream<Item = Result<ConfigResource, CliError>>,
{
    decode_config_stream_with_progress_timeout(
        resources,
        batch_size,
        pipeline_depth,
        limits,
        progress,
        QUERY_TIMEOUT,
    )
    .await
}

async fn decode_config_stream_with_progress_timeout<S>(
    resources: S,
    batch_size: usize,
    pipeline_depth: usize,
    limits: ConfigDecodeLimits,
    progress: &mut MetadataProgress,
    progress_timeout: Duration,
) -> Result<ConfigMetadata, CliError>
where
    S: Stream<Item = Result<ConfigResource, CliError>>,
{
    if limits.in_flight_bytes == 0
        || limits.resource_bytes == 0
        || limits.batch_bytes == 0
        || limits.total_bytes == 0
    {
        return Err(CliError::Data(
            "Config decoding limits must be greater than zero".to_owned(),
        ));
    }
    let in_flight = Arc::new(Semaphore::new(limits.in_flight_bytes));
    let jobs = resources
        .chunks(batch_size.max(1))
        .map(|batch| {
            let in_flight = Arc::clone(&in_flight);
            async move {
                let batch = batch.into_iter().collect::<Result<Vec<_>, _>>()?;
                let reserved_bytes = limits.batch_bytes.min(limits.in_flight_bytes);
                let reserved_permits = u32::try_from(reserved_bytes).map_err(|_| {
                    CliError::Data("Config in-flight byte limit exceeds semaphore range".to_owned())
                })?;
                let permit = in_flight
                    .acquire_many_owned(reserved_permits)
                    .await
                    .map_err(|_| CliError::Data("Config decoder semaphore closed".to_owned()))?;
                tokio::task::spawn_blocking(move || {
                    let mut result = DecodedConfigBatch {
                        resource_count: batch.len(),
                        compressed_bytes: batch
                            .iter()
                            .map(|resource| resource.compressed.len())
                            .sum(),
                        decoded_bytes: 0,
                        descriptors: Vec::new(),
                        predefined_values: Vec::new(),
                        criteria: Vec::new(),
                        roles: Vec::new(),
                        skipped: Vec::new(),
                    };
                    for resource in batch {
                        let remaining = limits.batch_bytes.saturating_sub(result.decoded_bytes);
                        if remaining == 0 {
                            return Err(CliError::Data(format!(
                                "Config batch decoded size exceeds {} bytes",
                                limits.batch_bytes
                            )));
                        }
                        // A resource the decoder cannot read — a `.7` of some
                        // charts of characteristic types is a binary format,
                        // not the brace text — is skipped with a warning; it
                        // carries no name the query language needs.
                        let mut parsed = match parse_config_resource_bounded(
                            &resource.file_name,
                            &resource.compressed,
                            limits.resource_bytes.min(remaining),
                        ) {
                            Ok(parsed) => parsed,
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    MetadataErrorKind::Utf8 | MetadataErrorKind::Serialization
                                ) =>
                            {
                                result
                                    .skipped
                                    .push(format!("{}: {error}", resource.file_name));
                                continue;
                            }
                            Err(error) => return Err(error.into()),
                        };
                        result.decoded_bytes = result
                            .decoded_bytes
                            .checked_add(parsed.decoded_bytes)
                            .ok_or_else(|| {
                                CliError::Data("Config decoded byte count overflowed".to_owned())
                            })?;
                        result.descriptors.append(&mut parsed.descriptors);
                        result
                            .predefined_values
                            .append(&mut parsed.predefined_values);
                        result.criteria.extend(parsed.criterion);
                        result.roles.append(&mut parsed.roles);
                    }
                    drop(permit);
                    Ok::<_, CliError>(result)
                })
                .await
                .map_err(|error| CliError::Data(format!("Config decoder worker failed: {error}")))?
            }
        })
        .buffer_unordered(pipeline_depth.max(1));
    tokio::pin!(jobs);

    let mut total_decoded_bytes = 0_usize;
    let mut descriptors = Vec::new();
    let mut predefined_values = Vec::new();
    let mut criteria = Vec::new();
    let mut roles = Vec::new();
    let mut skipped = Vec::new();
    loop {
        let next = timeout(progress_timeout, jobs.next()).await.map_err(|_| {
            CliError::DatabaseTimeout {
                operation: "Config stream progress".to_owned(),
                duration: progress_timeout,
            }
        })?;
        let Some(result) = next else {
            break;
        };
        let mut batch = result?;
        total_decoded_bytes = total_decoded_bytes
            .checked_add(batch.decoded_bytes)
            .ok_or_else(|| CliError::Data("Config decoded byte count overflowed".to_owned()))?;
        if total_decoded_bytes > limits.total_bytes {
            return Err(CliError::Data(format!(
                "Config total decoded size exceeds {} bytes",
                limits.total_bytes
            )));
        }
        progress.advance_config(batch.resource_count, batch.compressed_bytes);
        descriptors.append(&mut batch.descriptors);
        predefined_values.append(&mut batch.predefined_values);
        criteria.append(&mut batch.criteria);
        roles.append(&mut batch.roles);
        skipped.append(&mut batch.skipped);
    }
    if !skipped.is_empty() {
        eprintln!(
            "warning: skipped {} unreadable Config resources: {}",
            skipped.len(),
            skipped
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    descriptors.sort_by(|left, right| {
        left.resource_guid
            .as_str()
            .cmp(right.resource_guid.as_str())
            .then_with(|| left.object_guid.as_str().cmp(right.object_guid.as_str()))
            .then_with(|| left.name.cmp(&right.name))
    });
    predefined_values.sort_by(|left, right| {
        left.owner_guid
            .as_str()
            .cmp(right.owner_guid.as_str())
            .then_with(|| left.value_guid.as_str().cmp(right.value_guid.as_str()))
            .then_with(|| left.name.cmp(&right.name))
    });
    criteria.sort_by(|left, right| left.name.cmp(&right.name));
    Ok((descriptors, predefined_values, criteria, roles))
}

pub(crate) async fn run_metadata_blocking<T, F>(label: &'static str, work: F) -> Result<T, CliError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CliError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| CliError::Data(format!("{label} processing worker failed: {error}")))?
}

pub(crate) fn config_pipeline_depth() -> usize {
    std::thread::available_parallelism().map_or(4, |parallelism| {
        parallelism.get().saturating_mul(2).clamp(2, 16)
    })
}

pub(crate) fn unsigned_progress_total(value: i64, label: &str) -> Result<u64, CliError> {
    u64::try_from(value)
        .map_err(|_| CliError::Data(format!("database returned a negative Config {label}")))
}

#[cfg(test)]
#[path = "tests/pipeline.rs"]
mod tests;
