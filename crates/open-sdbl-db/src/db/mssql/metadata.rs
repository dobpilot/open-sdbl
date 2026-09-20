//! Reading 1C metadata through a SQL Server session.

use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, MsSqlMetadataQueries, StorageLayout, parse_db_names, parse_schema_storage,
};
use open_sdbl::query::MsSqlBackend;
use tiberius::Client as MsSqlClient;
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use crate::error::DbError;
use crate::limits::Limits;
use crate::pipeline::{
    ConfigDecodeLimits, ConfigMetadata, ConfigResource, MetadataSource, assemble_parts,
    assemble_single_resource, config_pipeline_depth, decode_catalog_values, decode_config_stream,
    run_metadata_blocking, unsigned_progress_total,
};
use crate::progress::MetadataProgress;
use crate::session::query_timeout;

type MsSqlTransport = Compat<TcpStream>;
use super::session::MsSqlSession;

#[cfg(test)]
#[path = "../../tests/mssql_live.rs"]
mod tests;

pub(super) struct MsSqlMetadataSource<'session> {
    session: &'session mut MsSqlSession,
}

impl<'session> MsSqlMetadataSource<'session> {
    pub(super) fn new(session: &'session mut MsSqlSession) -> Self {
        Self { session }
    }
}

impl MetadataSource for MsSqlMetadataSource<'_> {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        self.session.execute_batch("BEGIN TRANSACTION").await
    }

    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        let limits = self.session.limits();
        let rows = mssql_rows(
            self.session.client_mut()?,
            limits,
            "MSSQL storage layout query",
            MsSqlMetadataQueries::LAYOUT,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "storage layout")?;
        let mut flags = [0_i32; 6];
        for (index, flag) in flags.iter_mut().enumerate() {
            *flag = required_mssql_i32(row, index, "storage layout flag")?;
        }
        Ok(StorageLayout::from_flags(flags))
    }

    async fn read_db_names(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<open_sdbl::metadata::DbNames, DbError> {
        let limits = self.session.limits();
        let rows = mssql_rows(
            self.session.client_mut()?,
            limits,
            "MSSQL DBNames query",
            MsSqlMetadataQueries::db_names(layout),
        )
        .await?;
        let parts = rows
            .iter()
            .map(|row| {
                Ok((
                    required_mssql_i32(row, 0, "DBNames part number")?,
                    required_mssql_bytes(row, 1, "DBNames payload")?,
                ))
            })
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
        let limits = self.session.limits();
        let client = self.session.client_mut()?;
        let totals = mssql_rows(
            client,
            limits,
            "MSSQL Config totals query",
            MsSqlMetadataQueries::CONFIG_TOTALS,
        )
        .await?;
        let totals = exactly_one_mssql_row(&totals, "Config totals")?;
        progress.config_totals(
            unsigned_progress_total(
                required_mssql_i64(totals, 0, "Config resource count")?,
                "resource count",
            )?,
            unsigned_progress_total(
                required_mssql_i64(totals, 1, "Config compressed byte count")?,
                "compressed byte count",
            )?,
        );

        let rows = query_timeout(limits, "MSSQL Config query", async {
            client
                .simple_query(MsSqlMetadataQueries::config(layout))
                .await
                .map_err(DbError::mssql_query)
        })
        .await?
        .into_row_stream();
        let parts = rows.map(|row| {
            let row = row.map_err(DbError::mssql_query)?;
            Ok((
                required_mssql_string(&row, 0, "Config file name")?,
                required_mssql_i32(&row, 1, "Config part number")?,
                required_mssql_bytes(&row, 2, "Config payload")?,
            ))
        });
        decode_config_stream(
            assemble_parts(parts),
            limits.config_decode_batch_size,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
            limits.query_timeout,
        )
        .await
    }

    async fn read_extension_resources(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        let Some(query) = MsSqlMetadataQueries::extension_resources(layout) else {
            return Ok(Vec::new());
        };
        let limits = self.session.limits();
        let rows = query_timeout(limits, "MSSQL ConfigCAS query", async {
            self.session
                .client_mut()?
                .simple_query(query)
                .await
                .map_err(DbError::mssql_query)
        })
        .await?
        .into_row_stream();
        let parts = rows.map(|row| {
            let row = row.map_err(DbError::mssql_query)?;
            Ok((
                required_mssql_string(&row, 0, "ConfigCAS file name")?,
                required_mssql_i32(&row, 1, "ConfigCAS part number")?,
                required_mssql_bytes(&row, 2, "ConfigCAS payload")?,
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
        let Some(query) = MsSqlMetadataQueries::extension_restructure(layout) else {
            return Ok(Vec::new());
        };
        let limits = self.session.limits();
        let rows = mssql_rows(
            self.session.client_mut()?,
            limits,
            "MSSQL extension restructure query",
            query,
        )
        .await?;
        rows.iter()
            .map(|row| required_mssql_bytes(row, 0, "extension restructure payload"))
            .collect()
    }

    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, DbError> {
        let limits = self.session.limits();
        let rows = mssql_rows(
            self.session.client_mut()?,
            limits,
            "MSSQL SchemaStorage query",
            MsSqlMetadataQueries::SCHEMA,
        )
        .await?;
        let data = required_mssql_bytes(
            exactly_one_mssql_row(&rows, "SchemaStorage")?,
            0,
            "SchemaStorage payload",
        )?;
        run_metadata_blocking("SchemaStorage", move || {
            parse_schema_storage(&data).map_err(DbError::from)
        })
        .await
    }

    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
        let limits = self.session.limits();
        let rows = mssql_rows(
            self.session.client_mut()?,
            limits,
            "MSSQL catalog query",
            MsSqlMetadataQueries::CATALOG,
        )
        .await?;
        let mut values = Vec::with_capacity(rows.len());
        for row in &rows {
            values.push([
                required_mssql_string(row, 0, "catalog row tag")?,
                required_mssql_string(row, 1, "catalog table name")?,
                required_mssql_string(row, 2, "catalog value")?,
                required_mssql_string(row, 3, "catalog detail")?,
                required_mssql_string(row, 4, "catalog columns")?,
            ]);
        }
        run_metadata_blocking("MSSQL catalog", move || decode_catalog_values(values)).await
    }

    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        match self.session.execute_batch("COMMIT TRANSACTION").await {
            Ok(()) => Ok(()),
            Err(error) => Err(self.session.rollback_after_error(error).await),
        }
    }

    async fn rollback_readonly(&mut self, original: DbError) -> DbError {
        self.session.rollback_after_error(original).await
    }
}

pub(super) async fn mssql_rows(
    client: &mut MsSqlClient<MsSqlTransport>,
    limits: Limits,
    label: &str,
    sql: &str,
) -> Result<Vec<tiberius::Row>, DbError> {
    query_timeout(limits, label, async {
        client
            .simple_query(sql)
            .await
            .map_err(DbError::mssql_query)?
            .into_first_result()
            .await
            .map_err(DbError::mssql_query)
    })
    .await
}

pub(crate) fn apply_mssql_cleanup(
    poisoned: &mut bool,
    rollback: Result<(), String>,
    transaction_count: Result<i32, String>,
) -> Result<(), String> {
    if let Err(error) = rollback {
        *poisoned = true;
        return Err(format!("ROLLBACK failed: {error}"));
    }
    match transaction_count {
        Ok(0) => Ok(()),
        Ok(count) => {
            *poisoned = true;
            Err(format!("@@TRANCOUNT remained {count} after ROLLBACK"))
        }
        Err(error) => {
            *poisoned = true;
            Err(format!("cannot verify @@TRANCOUNT after ROLLBACK: {error}"))
        }
    }
}

pub(super) fn exactly_one_mssql_row<'rows>(
    rows: &'rows [tiberius::Row],
    name: &str,
) -> Result<&'rows tiberius::Row, DbError> {
    match rows {
        [row] => Ok(row),
        [] => Err(DbError::Data(format!("{name} resource is missing"))),
        _ => Err(DbError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

pub(super) fn required_mssql_string(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<String, DbError> {
    row.try_get::<&str, _>(index)
        .map(|value| value.map(str::to_owned))
        .map_err(DbError::mssql_query)?
        .ok_or_else(|| DbError::Data(format!("MSSQL returned NULL for {name}")))
}

pub(super) fn required_mssql_bytes(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<Vec<u8>, DbError> {
    row.try_get::<&[u8], _>(index)
        .map(|value| value.map(<[u8]>::to_vec))
        .map_err(DbError::mssql_query)?
        .ok_or_else(|| DbError::Data(format!("MSSQL returned NULL for {name}")))
}

pub(super) fn required_mssql_i64(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<i64, DbError> {
    row.try_get::<i64, _>(index)
        .map_err(DbError::mssql_query)?
        .ok_or_else(|| DbError::Data(format!("MSSQL returned NULL for {name}")))
}

pub(super) fn required_mssql_i32(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<i32, DbError> {
    row.try_get::<i32, _>(index)
        .map_err(DbError::mssql_query)?
        .ok_or_else(|| DbError::Data(format!("MSSQL returned NULL for {name}")))
}

impl MsSqlSession {
    pub(crate) const fn backend(&self) -> MsSqlBackend {
        self.backend
    }

    /// Whether the session can no longer be used: a failure poisoned it,
    /// or a streaming read was stopped and the connection carrying the
    /// statement was dropped to end it.
    #[must_use]
    pub const fn is_dead(&self) -> bool {
        self.poisoned
    }
}
