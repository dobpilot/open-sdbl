use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};

use crate::cache::MetadataCache;
use crate::config::ServiceConfig;
use crate::error::{ErrorCode, ServiceError};
use crate::model::{
    MetadataIssue, ScanRequest, SdblPrepareRequest, SdblPrepareResponse, SdblScanRequest,
    TableMetadata,
};
use crate::postgres::{execute_scan, execute_sdbl_scan, prepare_sdbl_query};
use crate::query::prepare_scan;

#[derive(Default)]
struct Metrics {
    queries_total: AtomicU64,
    query_errors_total: AtomicU64,
    rows_returned_total: AtomicU64,
    query_duration_micros: AtomicU64,
}

#[derive(Clone)]
pub struct AppState {
    cache: Arc<MetadataCache>,
    pool: Pool,
    config: ServiceConfig,
    metrics: Arc<Metrics>,
}

impl AppState {
    #[must_use]
    pub fn new(cache: Arc<MetadataCache>, pool: Pool, config: ServiceConfig) -> Self {
        Self {
            cache,
            pool,
            config,
            metrics: Arc::new(Metrics::default()),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/v1/metadata/schemas", get(schemas))
        .route("/v1/metadata/tables", get(tables))
        .route("/v1/metadata/table", get(table))
        .route("/v1/metadata/issues", get(issues))
        .route("/v1/metadata/refresh", post(refresh))
        .route("/v1/scan", post(scan))
        .route("/v1/sdbl/prepare", post(prepare_sdbl))
        .route("/v1/sdbl/scan", post(scan_sdbl))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok\n"
}

async fn ready(State(state): State<AppState>) -> Response {
    if state.cache.ready().await {
        (StatusCode::OK, "ready\n").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "metadata is not ready\n").into_response()
    }
}

async fn schemas(State(state): State<AppState>) -> Result<Json<Vec<String>>, ServiceError> {
    Ok(Json(state.cache.get().await?.catalog.schemas.clone()))
}

#[derive(Deserialize)]
struct SchemaQuery {
    schema: Option<String>,
}

async fn tables(
    State(state): State<AppState>,
    Query(query): Query<SchemaQuery>,
) -> Result<Json<Vec<TableMetadata>>, ServiceError> {
    let generation = state.cache.get().await?;
    Ok(Json(
        generation
            .catalog
            .tables
            .iter()
            .filter(|table| {
                query
                    .schema
                    .as_ref()
                    .is_none_or(|schema| table.schema == *schema)
            })
            .cloned()
            .collect(),
    ))
}

#[derive(Deserialize)]
struct TableQuery {
    schema: String,
    table: String,
}

async fn table(
    State(state): State<AppState>,
    Query(query): Query<TableQuery>,
) -> Result<Json<TableMetadata>, ServiceError> {
    let generation = state.cache.get().await?;
    generation
        .catalog
        .table(&query.schema, &query.table)
        .cloned()
        .map(Json)
        .ok_or_else(|| {
            ServiceError::new(
                ErrorCode::ObjectNotFound,
                format!("object {:?}.{:?} does not exist", query.schema, query.table),
            )
        })
}

async fn issues(State(state): State<AppState>) -> Result<Json<Vec<MetadataIssue>>, ServiceError> {
    Ok(Json(state.cache.get().await?.catalog.issues.clone()))
}

async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ServiceError> {
    let Some(token) = state.config.refresh_token.as_deref() else {
        return Ok(StatusCode::NOT_FOUND);
    };
    let expected = format!("Bearer {token}");
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        != Some(expected.as_str())
    {
        return Ok(StatusCode::UNAUTHORIZED);
    }
    state.cache.force_refresh().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn scan(
    State(state): State<AppState>,
    Json(request): Json<ScanRequest>,
) -> Result<Response, ServiceError> {
    if let Some(maximum) = state.config.maximum_result_rows
        && request.limit.is_none_or(|limit| limit > maximum)
    {
        return Err(ServiceError::new(
            ErrorCode::ResultLimit,
            format!("scan requires LIMIT no greater than configured maximum {maximum}"),
        ));
    }
    let generation = state.cache.get().await?;
    let table = generation
        .catalog
        .table(&request.schema, &request.table)
        .ok_or_else(|| {
            ServiceError::new(
                ErrorCode::ObjectNotFound,
                format!(
                    "object {:?}.{:?} does not exist",
                    request.schema, request.table
                ),
            )
        })?;
    let prepared = prepare_scan(table, &request)?;
    let selected_fields = request.columns.clone();
    let pushed_predicates = request.filters.len();
    let logical_object = format!("{}.{}", request.schema, request.table);
    let sql = prepared.sql.clone();
    debug!(
        logical_object,
        ?selected_fields,
        pushed_predicates,
        generated_postgres_sql = sql,
        "Trino scan prepared"
    );
    let pool = state.pool.clone();
    let timeout = state.config.query_timeout;
    let statement_timeout_ms =
        u64::try_from(state.config.statement_timeout.as_millis()).unwrap_or(u64::MAX);
    let metrics = Arc::clone(&state.metrics);
    metrics.queries_total.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(16);
    tokio::spawn(async move {
        let started = Instant::now();
        let result = tokio::time::timeout(
            timeout,
            execute_scan(&pool, &prepared, statement_timeout_ms, |row| {
                let sender = sender.clone();
                async move {
                    let mut line =
                        serde_json::to_vec(&StreamMessage::Row { row }).map_err(|error| {
                            ServiceError::new(ErrorCode::Internal, error.to_string())
                        })?;
                    line.push(b'\n');
                    sender.send(Ok(Bytes::from(line))).await.map_err(|_| {
                        ServiceError::new(ErrorCode::Protocol, "Trino closed the scan stream")
                    })
                }
            }),
        )
        .await;
        match result {
            Ok(Ok(rows)) => {
                metrics
                    .rows_returned_total
                    .fetch_add(rows, Ordering::Relaxed);
                let _ = send_message(&sender, &StreamMessage::Stats { rows }).await;
                debug!(
                    logical_object,
                    ?selected_fields,
                    pushed_predicates,
                    generated_postgres_sql = sql,
                    duration_ms = started.elapsed().as_millis(),
                    rows,
                    "Trino scan completed"
                );
            }
            Ok(Err(service_error)) => {
                metrics.query_errors_total.fetch_add(1, Ordering::Relaxed);
                error!(error = %service_error, logical_object, "Trino scan failed");
                let _ = send_message(
                    &sender,
                    &StreamMessage::Error {
                        code: service_error.code,
                        message: service_error.message,
                        retryable: service_error.retryable,
                    },
                )
                .await;
            }
            Err(_) => {
                metrics.query_errors_total.fetch_add(1, Ordering::Relaxed);
                let _ = send_message(
                    &sender,
                    &StreamMessage::Error {
                        code: ErrorCode::Timeout,
                        message: "query execution timed out".to_owned(),
                        retryable: false,
                    },
                )
                .await;
            }
        }
        metrics.query_duration_micros.fetch_add(
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    });
    Ok((
        [(header::CONTENT_TYPE, "application/x-ndjson")],
        Body::from_stream(ReceiverStream::new(receiver)),
    )
        .into_response())
}

async fn prepare_sdbl(
    State(state): State<AppState>,
    Json(request): Json<SdblPrepareRequest>,
) -> Result<Json<SdblPrepareResponse>, ServiceError> {
    if request.query.trim().is_empty() {
        return Err(ServiceError::new(
            ErrorCode::Compilation,
            "SDBL query must not be empty",
        ));
    }
    let generation = state.cache.get().await?;
    let prepared = tokio::time::timeout(
        state.config.query_timeout,
        prepare_sdbl_query(&state.pool, &generation.snapshot, &request.query),
    )
    .await
    .map_err(|_| {
        ServiceError::new(ErrorCode::Timeout, "SDBL table-function analysis timed out")
    })??;
    Ok(Json(SdblPrepareResponse {
        columns: prepared.columns,
    }))
}

async fn scan_sdbl(
    State(state): State<AppState>,
    Json(request): Json<SdblScanRequest>,
) -> Result<Response, ServiceError> {
    if let Some(maximum) = state.config.maximum_result_rows
        && request.limit.is_none_or(|limit| limit > maximum)
    {
        return Err(ServiceError::new(
            ErrorCode::ResultLimit,
            format!("SDBL scan requires LIMIT no greater than configured maximum {maximum}"),
        ));
    }
    if request.expected_columns.is_empty() {
        return Err(ServiceError::new(
            ErrorCode::Protocol,
            "SDBL scan has no analyzed result columns",
        ));
    }
    let generation = state.cache.get().await?;
    let snapshot = Arc::clone(&generation.snapshot);
    let selected_fields = request
        .columns
        .iter()
        .map(|index| {
            request
                .expected_columns
                .get(*index)
                .map_or_else(|| format!("#{index}"), |column| column.name.clone())
        })
        .collect::<Vec<_>>();
    debug!(
        ?selected_fields,
        limit = request.limit,
        "SDBL scan prepared"
    );
    let pool = state.pool.clone();
    let timeout = state.config.query_timeout;
    let statement_timeout_ms =
        u64::try_from(state.config.statement_timeout.as_millis()).unwrap_or(u64::MAX);
    let metrics = Arc::clone(&state.metrics);
    metrics.queries_total.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(16);
    tokio::spawn(async move {
        let started = Instant::now();
        let result = tokio::time::timeout(
            timeout,
            execute_sdbl_scan(&pool, &snapshot, &request, statement_timeout_ms, |row| {
                let sender = sender.clone();
                async move {
                    let mut line =
                        serde_json::to_vec(&StreamMessage::Row { row }).map_err(|error| {
                            ServiceError::new(ErrorCode::Internal, error.to_string())
                        })?;
                    line.push(b'\n');
                    sender.send(Ok(Bytes::from(line))).await.map_err(|_| {
                        ServiceError::new(ErrorCode::Protocol, "Trino closed the SDBL stream")
                    })
                }
            }),
        )
        .await;
        match result {
            Ok(Ok(rows)) => {
                metrics
                    .rows_returned_total
                    .fetch_add(rows, Ordering::Relaxed);
                let _ = send_message(&sender, &StreamMessage::Stats { rows }).await;
                debug!(
                    ?selected_fields,
                    duration_ms = started.elapsed().as_millis(),
                    rows,
                    "SDBL scan completed"
                );
            }
            Ok(Err(service_error)) => {
                metrics.query_errors_total.fetch_add(1, Ordering::Relaxed);
                error!(error = %service_error, "SDBL scan failed");
                let _ = send_message(
                    &sender,
                    &StreamMessage::Error {
                        code: service_error.code,
                        message: service_error.message,
                        retryable: service_error.retryable,
                    },
                )
                .await;
            }
            Err(_) => {
                metrics.query_errors_total.fetch_add(1, Ordering::Relaxed);
                let _ = send_message(
                    &sender,
                    &StreamMessage::Error {
                        code: ErrorCode::Timeout,
                        message: "SDBL query execution timed out".to_owned(),
                        retryable: false,
                    },
                )
                .await;
            }
        }
        metrics.query_duration_micros.fetch_add(
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    });
    Ok(stream_response(receiver))
}

fn stream_response(receiver: mpsc::Receiver<Result<Bytes, Infallible>>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/x-ndjson")],
        Body::from_stream(ReceiverStream::new(receiver)),
    )
        .into_response()
}

async fn send_message(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    message: &StreamMessage,
) -> Result<(), ()> {
    let mut line = serde_json::to_vec(message).map_err(|_| ())?;
    line.push(b'\n');
    sender.send(Ok(Bytes::from(line))).await.map_err(|_| ())
}

async fn metrics(State(state): State<AppState>) -> String {
    let (metadata_refresh_total, metadata_refresh_errors_total) = state.cache.refresh_counts();
    format!(
        concat!(
            "# TYPE metadata_refresh_total counter\n",
            "metadata_refresh_total {}\n",
            "# TYPE metadata_refresh_errors_total counter\n",
            "metadata_refresh_errors_total {}\n",
            "# TYPE queries_total counter\n",
            "queries_total {}\n",
            "# TYPE query_errors_total counter\n",
            "query_errors_total {}\n",
            "# TYPE rows_returned_total counter\n",
            "rows_returned_total {}\n",
            "# TYPE query_duration_seconds summary\n",
            "query_duration_seconds_count {}\n",
            "query_duration_seconds_sum {}\n",
            "# TYPE postgres_connections gauge\n",
            "postgres_connections {}\n"
        ),
        metadata_refresh_total,
        metadata_refresh_errors_total,
        state.metrics.queries_total.load(Ordering::Relaxed),
        state.metrics.query_errors_total.load(Ordering::Relaxed),
        state.metrics.rows_returned_total.load(Ordering::Relaxed),
        state.metrics.queries_total.load(Ordering::Relaxed),
        state.metrics.query_duration_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0,
        state.pool.status().size,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum StreamMessage {
    Row {
        row: Vec<Option<String>>,
    },
    Stats {
        rows: u64,
    },
    Error {
        code: ErrorCode,
        message: String,
        retryable: bool,
    },
}
