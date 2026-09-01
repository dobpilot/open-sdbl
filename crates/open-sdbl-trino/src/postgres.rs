use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use futures_util::{Stream, StreamExt};
use open_sdbl::metadata::{
    ConfigDescriptor, LiveColumn, LiveIndex, LiveTable, MetadataSnapshot, PostgresMetadataQueries,
    parse_config_descriptors, parse_db_names, parse_schema_storage, resolve_metadata,
};
use open_sdbl::query::{CompiledQuery, default_presentation_plan, prepare_postgres_query};
use tokio_postgres::types::ToSql;
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};
use tracing::debug;

use crate::config::{DatabaseTls, ServiceConfig};
use crate::error::{ErrorCode, ServiceError};
use crate::model::{SdblColumnMetadata, SdblScanRequest};
use crate::query::{PreparedScan, quote_identifier};
use crate::types::{TrinoType, map_statement_type};

/// One compiled and PostgreSQL-described SDBL SELECT.
pub struct PreparedSdblQuery {
    compiled: CompiledQuery,
    pub columns: Vec<SdblColumnMetadata>,
    types: Vec<TrinoType>,
}

/// Creates the bounded PostgreSQL pool used by metadata and data reads.
pub fn create_pool(config: &ServiceConfig) -> Result<Pool, ServiceError> {
    let manager_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };
    let manager = match config.tls {
        DatabaseTls::Disable => {
            Manager::from_config(config.database.clone(), NoTls, manager_config)
        }
        DatabaseTls::Native => {
            let connector = native_tls::TlsConnector::builder()
                .build()
                .map_err(|error| {
                    ServiceError::new(
                        ErrorCode::PostgresConnection,
                        format!("cannot initialize PostgreSQL TLS: {error}"),
                    )
                })?;
            Manager::from_config(
                config.database.clone(),
                postgres_native_tls::MakeTlsConnector::new(connector),
                manager_config,
            )
        }
    };
    Pool::builder(manager)
        .max_size(config.pool_size)
        .runtime(Runtime::Tokio1)
        .create_timeout(Some(config.pool_create_timeout))
        .wait_timeout(Some(config.pool_wait_timeout))
        .build()
        .map_err(|error| ServiceError::new(ErrorCode::PostgresConnection, error.to_string()))
}

/// Loads one complete snapshot in a verified read-only transaction.
pub async fn load_metadata(
    pool: &Pool,
    batch_size: usize,
) -> Result<MetadataSnapshot, ServiceError> {
    let mut client = pool.get().await.map_err(connection_error)?;
    let transaction = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_only(true)
        .start()
        .await
        .map_err(query_error)?;
    verify_transaction(&transaction).await?;

    let rows = transaction
        .query(PostgresMetadataQueries::DB_NAMES, &[])
        .await
        .map_err(query_error)?;
    let data: Vec<u8> = exactly_one_row(&rows, "DBNames")?
        .try_get(0)
        .map_err(query_error)?;
    let db_names = blocking("DBNames", move || parse_db_names(&data)).await?;

    let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
    let rows = transaction
        .query_raw(PostgresMetadataQueries::CONFIG, parameters)
        .await
        .map_err(query_error)?;
    let resources = rows.map(|row| {
        let row = row.map_err(query_error)?;
        Ok(ConfigResource {
            file_name: row.try_get(0).map_err(query_error)?,
            compressed: row.try_get(1).map_err(query_error)?,
        })
    });
    let descriptors = decode_config_stream(resources, batch_size).await?;

    let rows = transaction
        .query(PostgresMetadataQueries::SCHEMA, &[])
        .await
        .map_err(query_error)?;
    let data: Vec<u8> = exactly_one_row(&rows, "SchemaStorage")?
        .try_get(0)
        .map_err(query_error)?;
    let schema = blocking("SchemaStorage", move || parse_schema_storage(&data)).await?;

    let rows = transaction
        .query(PostgresMetadataQueries::CATALOG, &[])
        .await
        .map_err(query_error)?;
    let live_tables =
        blocking_service("PostgreSQL catalog", move || decode_catalog_rows(rows)).await?;

    let snapshot = blocking_service("metadata resolution", move || {
        Ok(resolve_metadata(db_names, descriptors, schema, live_tables))
    })
    .await?;
    transaction.commit().await.map_err(query_error)?;
    Ok(snapshot)
}

/// Executes a prepared scan and calls `send` once for every text-encoded row.
pub async fn execute_scan<F, Fut>(
    pool: &Pool,
    prepared: &PreparedScan,
    statement_timeout_ms: u64,
    mut send: F,
) -> Result<u64, ServiceError>
where
    F: FnMut(Vec<Option<String>>) -> Fut,
    Fut: Future<Output = Result<(), ServiceError>>,
{
    let mut client = pool.get().await.map_err(connection_error)?;
    let transaction = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_only(true)
        .start()
        .await
        .map_err(query_error)?;
    transaction
        .query_one(
            "SELECT set_config('statement_timeout', $1, true)",
            &[&statement_timeout_ms.to_string()],
        )
        .await
        .map_err(query_error)?;
    let parameters = prepared
        .parameters
        .iter()
        .map(|value| value as &(dyn ToSql + Sync))
        .collect::<Vec<_>>();
    let rows = transaction
        .query_raw(&prepared.sql, parameters)
        .await
        .map_err(query_error)?;
    tokio::pin!(rows);
    let mut count = 0_u64;
    while let Some(row) = rows.next().await {
        let row = row.map_err(query_error)?;
        let values = (0..prepared.columns.len())
            .map(|index| row.try_get(index).map_err(query_error))
            .collect::<Result<Vec<Option<String>>, _>>()?;
        send(values).await?;
        count = count.saturating_add(1);
    }
    transaction.commit().await.map_err(query_error)?;
    Ok(count)
}

/// Compiles a bounded SDBL SELECT and asks PostgreSQL for its result shape
/// without executing any result rows.
pub async fn prepare_sdbl_query(
    pool: &Pool,
    snapshot: &MetadataSnapshot,
    source: &str,
) -> Result<PreparedSdblQuery, ServiceError> {
    let compiled = compile_sdbl(snapshot, source)?;
    let mut client = pool.get().await.map_err(connection_error)?;
    let transaction = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_only(true)
        .start()
        .await
        .map_err(query_error)?;
    verify_transaction(&transaction).await?;
    let prepared = describe_sdbl(&transaction, compiled).await?;
    transaction.commit().await.map_err(query_error)?;
    Ok(prepared)
}

/// Recompiles, shape-checks, and streams one SDBL table-function scan.
pub async fn execute_sdbl_scan<F, Fut>(
    pool: &Pool,
    snapshot: &MetadataSnapshot,
    request: &SdblScanRequest,
    statement_timeout_ms: u64,
    mut send: F,
) -> Result<u64, ServiceError>
where
    F: FnMut(Vec<Option<String>>) -> Fut,
    Fut: Future<Output = Result<(), ServiceError>>,
{
    let compiled = compile_sdbl(snapshot, &request.query)?;
    let mut client = pool.get().await.map_err(connection_error)?;
    let transaction = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_only(true)
        .start()
        .await
        .map_err(query_error)?;
    verify_transaction(&transaction).await?;
    transaction
        .query_one(
            "SELECT set_config('statement_timeout', $1, true)",
            &[&statement_timeout_ms.to_string()],
        )
        .await
        .map_err(query_error)?;
    let prepared = describe_sdbl(&transaction, compiled).await?;
    if prepared.columns != request.expected_columns {
        return Err(ServiceError::new(
            ErrorCode::InvalidMetadata,
            "SDBL result shape changed after Trino analyzed the table function",
        ));
    }
    let (sql, parameters) = wrap_sdbl_scan(&prepared, &request.columns, request.limit)?;
    debug!(
        generated_postgres_sql = sql,
        "SDBL PostgreSQL scan prepared"
    );
    let parameters = parameters
        .iter()
        .map(|value| value as &(dyn ToSql + Sync))
        .collect::<Vec<_>>();
    let rows = transaction
        .query_raw(&sql, parameters)
        .await
        .map_err(query_error)?;
    tokio::pin!(rows);
    let mut count = 0_u64;
    while let Some(row) = rows.next().await {
        let row = row.map_err(query_error)?;
        let values = (0..request.columns.len().max(1))
            .map(|index| row.try_get(index).map_err(query_error))
            .collect::<Result<Vec<Option<String>>, _>>()?;
        send(values).await?;
        count = count.saturating_add(1);
    }
    transaction.commit().await.map_err(query_error)?;
    Ok(count)
}

fn compile_sdbl(snapshot: &MetadataSnapshot, source: &str) -> Result<CompiledQuery, ServiceError> {
    let prepared = prepare_postgres_query(source, snapshot).map_err(compilation_error)?;
    let plans = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| default_presentation_plan(snapshot, target.object))
        .collect::<Vec<_>>();
    prepared
        .compile(snapshot, &plans)
        .map_err(compilation_error)
}

async fn describe_sdbl(
    transaction: &Transaction<'_>,
    compiled: CompiledQuery,
) -> Result<PreparedSdblQuery, ServiceError> {
    let statement = transaction
        .prepare(&compiled.sql)
        .await
        .map_err(query_error)?;
    if statement.columns().is_empty() {
        return Err(ServiceError::new(
            ErrorCode::Compilation,
            "SDBL table function must return at least one column",
        ));
    }
    if statement.columns().len() != compiled.columns.len() {
        return Err(ServiceError::new(
            ErrorCode::Internal,
            "SDBL compiler and PostgreSQL disagree about the result column count",
        ));
    }
    let names = disambiguate_output_names(&compiled.columns);
    let types = statement
        .columns()
        .iter()
        .map(|column| map_statement_type(column.type_().name(), column.type_modifier()))
        .collect::<Vec<_>>();
    let columns = names
        .into_iter()
        .zip(&types)
        .enumerate()
        .map(|(index, (name, data_type))| SdblColumnMetadata {
            index,
            name,
            type_signature: data_type.signature(),
            nullable: true,
        })
        .collect();
    Ok(PreparedSdblQuery {
        compiled,
        columns,
        types,
    })
}

fn disambiguate_output_names(labels: &[String]) -> Vec<String> {
    let mut used = BTreeSet::new();
    labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let base = if label.trim().is_empty() {
                format!("_col{}", index + 1)
            } else {
                label.trim().to_owned()
            };
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !used.insert(candidate.to_lowercase()) {
                candidate = format!("{base}__{suffix}");
                suffix += 1;
            }
            candidate
        })
        .collect()
}

fn wrap_sdbl_scan(
    prepared: &PreparedSdblQuery,
    selected: &[usize],
    limit: Option<u64>,
) -> Result<(String, Vec<String>), ServiceError> {
    if let Some(index) = selected
        .iter()
        .copied()
        .find(|index| *index >= prepared.columns.len())
    {
        return Err(ServiceError::new(
            ErrorCode::ColumnNotFound,
            format!("SDBL result column index {index} does not exist"),
        ));
    }
    let private_columns = (0..prepared.columns.len())
        .map(|index| quote_identifier(&format!("__c{index}")))
        .collect::<Vec<_>>();
    let projections = selected
        .iter()
        .enumerate()
        .map(|(output, index)| {
            let value = format!(
                "{}.{}",
                quote_identifier("__open_sdbl_query"),
                private_columns[*index]
            );
            let value = match prepared.types[*index] {
                TrinoType::Varbinary => format!("encode({value}::bytea, 'base64')"),
                _ => format!("{value}::text"),
            };
            format!("{value} AS {}", quote_identifier(&format!("c{output}")))
        })
        .collect::<Vec<_>>();
    let projection = if projections.is_empty() {
        "NULL::text AS \"c0\"".to_owned()
    } else {
        projections.join(", ")
    };
    let mut sql = format!(
        "SELECT {projection} FROM ({}) AS {}({})",
        prepared.compiled.sql,
        quote_identifier("__open_sdbl_query"),
        private_columns.join(", ")
    );
    let mut parameters = Vec::new();
    if let Some(limit) = limit {
        parameters.push(limit.to_string());
        use std::fmt::Write as _;
        write!(sql, " LIMIT CAST($1::text AS bigint)").expect("writing to String cannot fail");
    }
    Ok((sql, parameters))
}

struct ConfigResource {
    file_name: String,
    compressed: Vec<u8>,
}

async fn decode_config_stream<S>(
    resources: S,
    batch_size: usize,
) -> Result<Vec<ConfigDescriptor>, ServiceError>
where
    S: Stream<Item = Result<ConfigResource, ServiceError>>,
{
    let jobs = resources
        .chunks(batch_size.max(1))
        .map(|batch| async move {
            let batch = batch.into_iter().collect::<Result<Vec<_>, _>>()?;
            tokio::task::spawn_blocking(move || {
                let mut decoded = Vec::with_capacity(batch.len());
                for resource in batch {
                    decoded.push((
                        resource.file_name.clone(),
                        parse_config_descriptors(&resource.file_name, &resource.compressed)
                            .map_err(metadata_error)?,
                    ));
                }
                Ok::<_, ServiceError>(decoded)
            })
            .await
            .map_err(|error| internal_worker("Config", error))?
        })
        .buffered(pipeline_depth());
    tokio::pin!(jobs);
    let mut decoded = Vec::new();
    while let Some(batch) = jobs.next().await {
        decoded.extend(batch?);
    }
    decoded.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(decoded
        .into_iter()
        .flat_map(|(_, descriptors)| descriptors)
        .collect())
}

fn pipeline_depth() -> usize {
    std::thread::available_parallelism().map_or(4, |parallelism| {
        parallelism.get().saturating_mul(2).clamp(2, 16)
    })
}

async fn verify_transaction(transaction: &Transaction<'_>) -> Result<(), ServiceError> {
    let row = transaction
        .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
        .await
        .map_err(query_error)?;
    let read_only: String = row.try_get(0).map_err(query_error)?;
    let isolation: String = row.try_get(1).map_err(query_error)?;
    if read_only != "on" || !isolation.eq_ignore_ascii_case("read committed") {
        return Err(ServiceError::new(
            ErrorCode::InvalidMetadata,
            format!(
                "unsafe PostgreSQL transaction mode: read_only={read_only:?}, isolation={isolation:?}"
            ),
        ));
    }
    Ok(())
}

fn exactly_one_row<'rows>(rows: &'rows [Row], name: &str) -> Result<&'rows Row, ServiceError> {
    match rows {
        [row] => Ok(row),
        [] => Err(ServiceError::new(
            ErrorCode::InvalidMetadata,
            format!("{name} resource is missing"),
        )),
        _ => Err(ServiceError::new(
            ErrorCode::InvalidMetadata,
            format!("more than one {name} resource was returned"),
        )),
    }
}

fn decode_catalog_rows(rows: Vec<Row>) -> Result<Vec<LiveTable>, ServiceError> {
    let mut tables = BTreeMap::<String, LiveTable>::new();
    for row in rows {
        let tag: String = row.try_get(0).map_err(query_error)?;
        let table_name: String = row.try_get(1).map_err(query_error)?;
        let value: String = row.try_get(2).map_err(query_error)?;
        let detail: String = row.try_get(3).map_err(query_error)?;
        let columns: String = row.try_get(4).map_err(query_error)?;
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
                return Err(ServiceError::new(
                    ErrorCode::InvalidMetadata,
                    format!("unknown PostgreSQL catalog row tag {tag:?}"),
                ));
            }
        }
    }
    Ok(tables.into_values().collect())
}

async fn blocking<T, F>(label: &'static str, work: F) -> Result<T, ServiceError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, open_sdbl::metadata::MetadataError> + Send + 'static,
{
    blocking_service(label, move || work().map_err(metadata_error)).await
}

async fn blocking_service<T, F>(label: &'static str, work: F) -> Result<T, ServiceError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ServiceError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| internal_worker(label, error))?
}

fn metadata_error(error: open_sdbl::metadata::MetadataError) -> ServiceError {
    ServiceError::new(ErrorCode::InvalidMetadata, error.to_string())
}

fn compilation_error(error: open_sdbl::query::QueryDiagnostic) -> ServiceError {
    ServiceError::new(ErrorCode::Compilation, error.to_string())
}

fn connection_error(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::new(
        ErrorCode::PostgresConnection,
        format!("PostgreSQL connection failed: {error}"),
    )
    .retryable(true)
}

fn query_error(error: tokio_postgres::Error) -> ServiceError {
    let detail = error.as_db_error().map_or_else(
        || error.to_string(),
        |database| {
            format!(
                "{} (SQLSTATE {})",
                database.message(),
                database.code().code()
            )
        },
    );
    ServiceError::new(
        ErrorCode::PostgresQuery,
        format!("PostgreSQL query failed: {detail}"),
    )
}

fn internal_worker(label: &str, error: impl std::fmt::Display) -> ServiceError {
    ServiceError::new(
        ErrorCode::Internal,
        format!("{label} processing worker failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use open_sdbl::query::CompiledQuery;
    use tokio_postgres::Config as PostgresConfig;

    use super::{PreparedSdblQuery, create_pool, disambiguate_output_names, wrap_sdbl_scan};
    use crate::config::{DatabaseTls, ServiceConfig};
    use crate::model::SdblColumnMetadata;
    use crate::types::TrinoType;

    #[test]
    fn pool_timeouts_are_backed_by_the_tokio_runtime() {
        let config = ServiceConfig {
            listen: "127.0.0.1:0".parse().expect("valid test address"),
            database: PostgresConfig::new(),
            tls: DatabaseTls::Disable,
            metadata_cache_ttl: Duration::from_secs(300),
            pool_size: 1,
            pool_create_timeout: Duration::from_secs(1),
            pool_wait_timeout: Duration::from_secs(1),
            statement_timeout: Duration::from_secs(60),
            query_timeout: Duration::from_secs(65),
            maximum_result_rows: None,
            config_decode_batch_size: 256,
            refresh_token: None,
        };

        create_pool(&config).expect("timeouts must have an asynchronous runtime");
    }

    #[test]
    fn sdbl_wrapper_projects_ordinals_and_binds_limit() {
        let prepared = PreparedSdblQuery {
            compiled: CompiledQuery {
                sql: "SELECT 1 AS first, 'value'::text AS second".to_owned(),
                columns: vec!["first".to_owned(), "second".to_owned()],
            },
            columns: vec![
                SdblColumnMetadata {
                    index: 0,
                    name: "first".to_owned(),
                    type_signature: "integer".to_owned(),
                    nullable: true,
                },
                SdblColumnMetadata {
                    index: 1,
                    name: "second".to_owned(),
                    type_signature: "varchar".to_owned(),
                    nullable: true,
                },
            ],
            types: vec![TrinoType::Integer, TrinoType::Varchar],
        };

        let (sql, parameters) = wrap_sdbl_scan(&prepared, &[1], Some(7)).unwrap();
        assert!(sql.starts_with("SELECT \"__open_sdbl_query\".\"__c1\"::text AS \"c0\""));
        assert!(!sql.starts_with("SELECT \"__open_sdbl_query\".\"__c0\""));
        assert!(sql.ends_with("LIMIT CAST($1::text AS bigint)"));
        assert_eq!(parameters, ["7"]);
    }

    #[test]
    fn sdbl_output_names_are_nonempty_and_unique_case_insensitively() {
        assert_eq!(
            disambiguate_output_names(&[String::new(), "Value".to_owned(), "value".to_owned(),]),
            ["_col1", "Value", "value__2"]
        );
    }
}
