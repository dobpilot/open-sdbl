//! Reading 1C metadata through a PostgreSQL session.

use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, PostgresMetadataQueries, StorageLayout, parse_db_names, parse_schema_storage,
};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Row, Transaction};

use super::session::PostgresSession;
use crate::error::DbError;
use crate::limits::Limits;
use crate::pipeline::{
    ConfigDecodeLimits, ConfigMetadata, ConfigResource, MetadataSource, assemble_parts,
    assemble_single_resource, config_pipeline_depth, decode_catalog_values, decode_config_stream,
    run_metadata_blocking, unsigned_progress_total,
};
use crate::progress::MetadataProgress;
use crate::session::query_timeout;

pub(super) struct PostgresMetadataSource<'transaction> {
    transaction: Option<Transaction<'transaction>>,
    /// `server_version_num`, read once the read-only transaction is verified.
    server_version: Option<i32>,
    limits: Limits,
}

impl<'transaction> PostgresMetadataSource<'transaction> {
    pub(super) fn new(transaction: Transaction<'transaction>, limits: Limits) -> Self {
        Self {
            transaction: Some(transaction),
            server_version: None,
            limits,
        }
    }

    pub(super) fn transaction(&self) -> Result<&Transaction<'transaction>, DbError> {
        self.transaction.as_ref().ok_or_else(|| {
            DbError::Database("PostgreSQL metadata transaction is closed".to_owned())
        })
    }
}

impl MetadataSource for PostgresMetadataSource<'_> {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        verify_transaction(self.transaction()?, self.limits).await?;
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL server version query",
            PostgresMetadataQueries::SERVER_VERSION,
        )
        .await?;
        self.server_version = Some(exactly_one_row(&rows, "server version")?.try_get(0)?);
        Ok(())
    }

    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL storage layout query",
            PostgresMetadataQueries::LAYOUT,
        )
        .await?;
        let row = exactly_one_row(&rows, "storage layout")?;
        let mut flags = [0_i32; 6];
        for (index, flag) in flags.iter_mut().enumerate() {
            *flag = row.try_get(index)?;
        }
        Ok(StorageLayout::from_flags(flags))
    }

    async fn read_db_names(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<open_sdbl::metadata::DbNames, DbError> {
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL DBNames query",
            PostgresMetadataQueries::db_names(layout),
        )
        .await?;
        let parts = rows
            .iter()
            .map(|row| Ok((row.try_get::<_, i32>(0)?, row.try_get::<_, Vec<u8>>(1)?)))
            .collect::<Result<Vec<_>, DbError>>()?;
        let data = assemble_single_resource("DBNames", parts)?;
        run_metadata_blocking("DBNames", move || {
            parse_db_names(&data).map_err(DbError::from)
        })
        .await
    }

    async fn read_config(
        &mut self,
        layout: &StorageLayout,
        progress: &mut dyn MetadataProgress,
    ) -> Result<ConfigMetadata, DbError> {
        let transaction = self.transaction()?;
        let totals = query_timeout(self.limits, "PostgreSQL Config totals query", async {
            transaction
                .query_one(PostgresMetadataQueries::CONFIG_TOTALS, &[])
                .await
                .map_err(DbError::from)
        })
        .await?;
        progress.config_totals(
            unsigned_progress_total(totals.try_get(0)?, "resource count")?,
            unsigned_progress_total(totals.try_get(1)?, "compressed byte count")?,
        );

        let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
        let rows = query_timeout(self.limits, "PostgreSQL Config query", async {
            transaction
                .query_raw(PostgresMetadataQueries::config(layout), parameters)
                .await
                .map_err(DbError::from)
        })
        .await?;
        let parts = rows.map(|row| {
            let row = row?;
            Ok((
                row.try_get::<_, String>(0)?,
                row.try_get::<_, i32>(1)?,
                row.try_get::<_, Vec<u8>>(2)?,
            ))
        });
        decode_config_stream(
            assemble_parts(parts),
            self.limits.config_decode_batch_size,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
            self.limits.query_timeout,
        )
        .await
    }

    async fn read_extension_resources(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        let Some(query) = PostgresMetadataQueries::extension_resources(layout) else {
            return Ok(Vec::new());
        };
        let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
        let rows = query_timeout(self.limits, "PostgreSQL ConfigCAS query", async {
            self.transaction()?
                .query_raw(query, parameters)
                .await
                .map_err(DbError::from)
        })
        .await?;
        let parts = rows.map(|row| {
            let row = row?;
            Ok((
                row.try_get::<_, String>(0)?,
                row.try_get::<_, i32>(1)?,
                row.try_get::<_, Vec<u8>>(2)?,
            ))
        });
        let mut resources = std::pin::pin!(assemble_parts(parts));
        let mut assembled = Vec::new();
        while let Some(resource) = resources.next().await {
            assembled.push(resource?);
        }
        Ok(assembled)
    }

    async fn read_extension_restructures(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<Vec<Vec<u8>>, DbError> {
        let Some(query) = PostgresMetadataQueries::extension_restructure(layout) else {
            return Ok(Vec::new());
        };
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL extension restructure query",
            query,
        )
        .await?;
        rows.iter()
            .map(|row| row.try_get::<_, Vec<u8>>(0).map_err(DbError::from))
            .collect()
    }

    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, DbError> {
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL SchemaStorage query",
            PostgresMetadataQueries::SCHEMA,
        )
        .await?;
        let data: Vec<u8> = exactly_one_row(&rows, "SchemaStorage")?.try_get(0)?;
        run_metadata_blocking("SchemaStorage", move || {
            parse_schema_storage(&data).map_err(DbError::from)
        })
        .await
    }

    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
        let server_version = self.server_version.ok_or_else(|| {
            DbError::Database(
                "PostgreSQL server version was not read before the catalog".to_owned(),
            )
        })?;
        let rows = postgres_rows(
            self.transaction()?,
            self.limits,
            "PostgreSQL catalog query",
            PostgresMetadataQueries::catalog(server_version),
        )
        .await?;
        run_metadata_blocking("PostgreSQL catalog", move || decode_catalog_rows(rows)).await
    }

    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        let transaction = self.transaction.take().ok_or_else(|| {
            DbError::Database("PostgreSQL metadata transaction is closed".to_owned())
        })?;
        query_timeout(self.limits, "PostgreSQL transaction commit", async {
            transaction.commit().await.map_err(DbError::from)
        })
        .await
    }

    async fn rollback_readonly(&mut self, original: DbError) -> DbError {
        if let Some(transaction) = self.transaction.take() {
            let _ = query_timeout(self.limits, "PostgreSQL transaction rollback", async {
                transaction.rollback().await.map_err(DbError::from)
            })
            .await;
        }
        original
    }
}

pub(super) async fn verify_transaction(
    transaction: &Transaction<'_>,
    limits: Limits,
) -> Result<(), DbError> {
    let transaction_mode = query_timeout(limits, "PostgreSQL read-only verification", async {
        transaction
            .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
            .await
            .map_err(DbError::from)
    })
    .await?;
    let read_only: String = transaction_mode.try_get(0)?;
    let isolation: String = transaction_mode.try_get(1)?;
    if read_only != "on" || !isolation.eq_ignore_ascii_case("read committed") {
        return Err(DbError::Data(format!(
            "unsafe PostgreSQL transaction mode: read_only={read_only:?}, isolation={isolation:?}"
        )));
    }
    Ok(())
}

pub(super) async fn postgres_rows(
    transaction: &Transaction<'_>,
    limits: Limits,
    label: &str,
    sql: &str,
) -> Result<Vec<Row>, DbError> {
    query_timeout(limits, label, async {
        transaction.query(sql, &[]).await.map_err(DbError::from)
    })
    .await
}

pub(super) fn exactly_one_row<'rows>(
    rows: &'rows [Row],
    name: &str,
) -> Result<&'rows Row, DbError> {
    match rows {
        [row] => Ok(row),
        [] => Err(DbError::Data(format!("{name} resource is missing"))),
        _ => Err(DbError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

pub(super) fn decode_catalog_rows(rows: Vec<Row>) -> Result<Vec<LiveTable>, DbError> {
    let values = rows
        .into_iter()
        .map(|row| {
            Ok([
                row.try_get(0)?,
                row.try_get(1)?,
                row.try_get(2)?,
                row.try_get(3)?,
                row.try_get(4)?,
            ])
        })
        .collect::<Result<Vec<_>, tokio_postgres::Error>>()?;
    decode_catalog_values(values)
}

impl PostgresSession {
    pub(crate) fn is_closed(&self) -> bool {
        self.client.is_closed()
    }

    pub(crate) fn cancellation(&self) -> tokio_postgres::CancelToken {
        self.client.cancel_token()
    }
}
