use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, MetadataSnapshot, MsSqlMetadataQueries, StorageLayout, parse_db_names,
    parse_schema_storage,
};
use open_sdbl::query::{MsSqlBackend, MsSqlDialectLevel};
use tiberius::{AuthMethod, Client as MsSqlClient, ColumnData, Config as MsSqlConfig};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use zeroize::Zeroizing;

use crate::args::MsSqlConnection;
use crate::auth::pgpass::Credentials;
use crate::cells::{
    Cell, DAYS_FROM_1900_TO_UNIX_EPOCH, DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH, DateTimeParts,
    format_scaled_integer,
};
use crate::error::CliError;
use crate::net::socks5::{connect_socks5, socks5_password};
use crate::pipeline::{
    ConfigDecodeLimits, ConfigMetadata, ConfigResource, MetadataSource, acquire_metadata,
    assemble_parts, assemble_single_resource, config_pipeline_depth, decode_catalog_values,
    decode_config_stream, run_metadata_blocking, unsigned_progress_total,
};
use crate::progress::MetadataProgress;
use crate::{
    CONFIG_DECODE_BATCH_SIZE, CONNECTION_TIMEOUT, MSSQL_TRANSACTION_COUNT, MSSQL_VERIFY_READONLY,
    QUERY_TIMEOUT, QueryRows, query_timeout,
};

type MsSqlTransport = Compat<TcpStream>;

#[derive(Clone)]
struct MsSqlSecrets {
    password: Zeroizing<String>,
    socks5_password: Option<Zeroizing<String>>,
}

pub(crate) struct MsSqlSession {
    client: Option<MsSqlClient<MsSqlTransport>>,
    connection: MsSqlConnection,
    database: String,
    backend: MsSqlBackend,
    /// `SERVERPROPERTY('ProductVersion')` of the connected server.
    product_version: String,
    poisoned: bool,
    secrets: MsSqlSecrets,
}

impl MsSqlSession {
    pub(crate) async fn connect(
        connection: &MsSqlConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        let secrets = MsSqlSecrets {
            password: credentials.mssql.required("MSSQL_PASSWORD")?,
            socks5_password: socks5_password(
                connection.options.socks5_proxy.as_ref(),
                &credentials.socks5,
            )?,
        };
        Self::connect_with_secrets(connection, secrets).await
    }

    async fn connect_with_secrets(
        connection: &MsSqlConnection,
        secrets: MsSqlSecrets,
    ) -> Result<Self, CliError> {
        let options = &connection.options;
        let mut configuration = MsSqlConfig::new();
        configuration.host(&options.host);
        configuration.port(options.port);
        configuration.database(&options.database);
        configuration.authentication(AuthMethod::sql_server(
            &options.user,
            secrets.password.as_str(),
        ));
        configuration.application_name("open-sdbl");
        configuration.readonly(true);
        if connection.trust_server_certificate {
            configuration.trust_cert();
        } else if let Some(path) = &connection.trust_ca_file {
            configuration.trust_cert_ca(path);
        }

        let stream = if let Some(proxy) = &options.socks5_proxy {
            connect_socks5(
                proxy,
                secrets.socks5_password.as_deref().map(String::as_str),
                &options.host,
                options.port,
            )
            .await?
        } else {
            timeout(
                CONNECTION_TIMEOUT,
                TcpStream::connect((options.host.as_str(), options.port)),
            )
            .await
            .map_err(|_| {
                CliError::Database(format!(
                    "MSSQL TCP connection timed out after {} seconds",
                    CONNECTION_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|error| {
                CliError::Io("cannot connect to MSSQL TCP endpoint".to_owned(), error)
            })?
        };
        stream.set_nodelay(true).map_err(|error| {
            CliError::Io("cannot configure MSSQL TCP connection".to_owned(), error)
        })?;
        let client = timeout(
            CONNECTION_TIMEOUT,
            MsSqlClient::connect(configuration, stream.compat_write()),
        )
        .await
        .map_err(|_| {
            CliError::Database(format!(
                "MSSQL startup timed out after {} seconds",
                CONNECTION_TIMEOUT.as_secs()
            ))
        })?
        .map_err(CliError::mssql_connection)?;
        let mut session = Self {
            client: Some(client),
            connection: connection.clone(),
            database: options.database.clone(),
            backend: MsSqlBackend::default(),
            product_version: String::new(),
            poisoned: false,
            secrets,
        };
        session
            .execute_batch(
                &format!(
                    "SET QUOTED_IDENTIFIER ON; SET TRANSACTION ISOLATION LEVEL READ COMMITTED; SET LOCK_TIMEOUT {};",
                    QUERY_TIMEOUT.as_millis()
                ),
            )
            .await?;
        session.verify_database().await?;
        session.product_version = session.read_product_version().await?;
        let dialect_level = match connection.dialect_level {
            Some(level) => level,
            None => MsSqlDialectLevel::from_product_version(&session.product_version).ok_or_else(
                || {
                    CliError::Data(format!(
                        "unsupported SQL Server product version {:?}; pass --mssql-dialect explicitly",
                        session.product_version
                    ))
                },
            )?,
        };
        let year_offset = session.read_year_offset().await?;
        session.backend = MsSqlBackend::new(year_offset)
            .map_err(|error| CliError::Data(error.to_string()))?
            .with_dialect_level(dialect_level);
        Ok(session)
    }

    fn client_mut(&mut self) -> Result<&mut MsSqlClient<MsSqlTransport>, CliError> {
        self.client.as_mut().ok_or_else(|| {
            CliError::Database("MSSQL connection is closed; reconnect required".to_owned())
        })
    }

    async fn execute_batch(&mut self, sql: &str) -> Result<(), CliError> {
        self.ensure_usable()?;
        let result = {
            let client = self.client_mut()?;
            query_timeout("MSSQL batch", async {
                client
                    .simple_query(sql)
                    .await
                    .map_err(CliError::mssql_query)?
                    .into_results()
                    .await
                    .map_err(CliError::mssql_query)?;
                Ok(())
            })
            .await
        };
        if result
            .as_ref()
            .is_err_and(CliError::requires_mssql_disconnect)
        {
            self.poison_and_drop();
        }
        result
    }

    async fn verify_database(&mut self) -> Result<(), CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL database verification",
            MsSqlMetadataQueries::VERIFY_DATABASE,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "database verification")?;
        let actual = required_mssql_string(row, 0, "database name")?;
        let status = required_mssql_string(row, 1, "database status")?;
        if !actual.eq_ignore_ascii_case(&self.database) || !status.eq_ignore_ascii_case("ONLINE") {
            return Err(CliError::Data(format!(
                "unexpected MSSQL database state: database={actual:?}, status={status:?}"
            )));
        }
        Ok(())
    }

    async fn read_product_version(&mut self) -> Result<String, CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL product version query",
            MsSqlMetadataQueries::PRODUCT_VERSION,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "product version")?;
        required_mssql_string(row, 0, "SQL Server product version")
    }

    /// One-line description of the connected server and the dialect level
    /// generated T-SQL targets, printed when the console starts.
    pub(crate) fn server_description(&self) -> String {
        format!(
            "MSSQL dialect: {} (server {})",
            self.backend.dialect_level(),
            self.product_version
        )
    }

    async fn read_year_offset(&mut self) -> Result<i32, CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL _YearOffset query",
            MsSqlMetadataQueries::YEAR_OFFSET,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "_YearOffset")?;
        let offset = row
            .try_get::<i32, _>(0)
            .map_err(CliError::mssql_query)?
            .ok_or_else(|| {
                CliError::Data("MSSQL returned NULL for _YearOffset.Offset".to_owned())
            })?;
        if !matches!(offset, 0 | 2000) {
            return Err(CliError::Data(format!(
                "unsupported MSSQL _YearOffset.Offset value {offset}; expected 0 or 2000"
            )));
        }
        Ok(offset)
    }

    fn ensure_usable(&self) -> Result<(), CliError> {
        if self.poisoned || self.client.is_none() {
            Err(CliError::Database(
                "MSSQL session is poisoned and cannot be reused; reconnect required".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    async fn verify_readonly(&mut self) -> Result<(), CliError> {
        self.ensure_usable()?;
        let rows = {
            let client = self.client_mut()?;
            mssql_rows(
                client,
                "MSSQL read-only verification",
                MSSQL_VERIFY_READONLY,
            )
            .await
        };
        if rows
            .as_ref()
            .is_err_and(CliError::requires_mssql_disconnect)
        {
            self.poison_and_drop();
        }
        let rows = rows?;
        let row = exactly_one_mssql_row(&rows, "read-only verification")?;
        let transaction_count = required_mssql_i32(row, 0, "@@TRANCOUNT")?;
        let read_only = required_mssql_i32(row, 1, "read-only role result")?;
        let isolation = required_mssql_i32(row, 2, "transaction isolation level")?;
        if transaction_count != 0 || read_only != 1 || isolation != 2 {
            return Err(CliError::Data(format!(
                "unsafe MSSQL session: transaction_count={transaction_count}, db_datareader_only={}, isolation_level={isolation}; use a login in db_datareader and not db_datawriter/db_owner/sysadmin",
                read_only == 1
            )));
        }
        Ok(())
    }

    async fn transaction_count(&mut self) -> Result<i32, CliError> {
        let rows = mssql_rows(
            self.client_mut()?,
            "MSSQL transaction-state verification",
            MSSQL_TRANSACTION_COUNT,
        )
        .await?;
        required_mssql_i32(
            exactly_one_mssql_row(&rows, "transaction-state verification")?,
            0,
            "@@TRANCOUNT",
        )
    }

    async fn rollback_after_error(&mut self, original: CliError) -> CliError {
        if should_disconnect_after_mssql_error(&original) {
            self.poison_and_drop();
            return original;
        }
        let rollback = self
            .execute_batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION")
            .await
            .map_err(|error| error.to_string());
        let transaction_count = if rollback.is_ok() {
            self.transaction_count()
                .await
                .map_err(|error| error.to_string())
        } else {
            Ok(0)
        };
        match apply_mssql_cleanup(&mut self.poisoned, rollback, transaction_count) {
            Ok(()) => original,
            Err(cleanup) => CliError::Database(format!(
                "{original}; MSSQL rollback cleanup failed and the session was poisoned: {cleanup}"
            )),
        }
    }

    fn poison_and_drop(&mut self) {
        self.poisoned = true;
        drop(self.client.take());
    }

    pub(crate) async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        acquire_metadata(&mut MsSqlMetadataSource::new(self)).await
    }

    /// Probes the service-table layout without starting a transaction; used
    /// by diagnostics and live tests.
    #[cfg(test)]
    pub(crate) async fn storage_layout(&mut self) -> Result<StorageLayout, CliError> {
        MsSqlMetadataSource::new(self).detect_layout().await
    }

    pub(crate) async fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> Result<QueryRows, CliError> {
        self.verify_readonly().await?;
        self.execute_batch("BEGIN TRANSACTION").await?;
        let client = self.client_mut()?;
        let result = query_timeout("MSSQL user query", async {
            let rows = client
                .simple_query(sql)
                .await
                .map_err(CliError::mssql_query)?
                .into_first_result()
                .await
                .map_err(CliError::mssql_query)?;
            rows.iter()
                .map(|row| mssql_row(row, column_count))
                .collect()
        })
        .await;
        match result {
            Ok(rows) => match self.execute_batch("COMMIT TRANSACTION").await {
                Ok(()) => Ok(rows),
                Err(error) => Err(self.rollback_after_error(error).await),
            },
            Err(error) => Err(self.rollback_after_error(error).await),
        }
    }

    pub(crate) async fn cancel_and_reconnect(&mut self) -> Result<(), CliError> {
        // Dropping an in-flight Tiberius future can leave the TDS stream between
        // protocol messages. Do not send cleanup commands on that connection.
        self.poison_and_drop();
        let connection = self.connection.clone();
        let replacement = Self::connect_with_secrets(&connection, self.secrets.clone()).await?;
        *self = replacement;
        Ok(())
    }

    pub(crate) async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        Ok(())
    }
}

fn should_disconnect_after_mssql_error(error: &CliError) -> bool {
    error.requires_mssql_disconnect()
}

fn mssql_row(row: &tiberius::Row, column_count: usize) -> Result<Vec<Cell>, CliError> {
    if row.columns().len() < column_count {
        return Err(CliError::Data(format!(
            "MSSQL returned {} columns, but {column_count} were expected",
            row.columns().len()
        )));
    }
    row.cells()
        .take(column_count)
        .map(|(_, data)| Ok(decode_mssql_cell(data)))
        .collect()
}

/// Decodes one TDS value into a typed [`Cell`].
fn decode_mssql_cell(data: &ColumnData<'_>) -> Cell {
    fn nullable<T>(value: Option<T>, convert: impl FnOnce(T) -> Cell) -> Cell {
        value.map_or(Cell::Null, convert)
    }

    match data {
        ColumnData::U8(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I16(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I32(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I64(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::F32(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::F64(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::Bit(value) => nullable(*value, Cell::Bool),
        ColumnData::String(value) => {
            nullable(value.as_deref(), |value| Cell::Text(value.to_owned()))
        }
        ColumnData::Guid(value) => nullable(*value, |guid| Cell::Uuid(*guid.as_bytes())),
        ColumnData::Binary(value) => {
            nullable(value.as_deref(), |value| Cell::Bytes(value.to_vec()))
        }
        ColumnData::Numeric(value) => nullable(*value, |value| {
            Cell::Number(format_scaled_integer(
                value.value(),
                u32::from(value.scale()),
            ))
        }),
        ColumnData::Xml(value) => nullable(value.as_deref(), |value| Cell::Text(value.to_string())),
        ColumnData::DateTime(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_1900_TO_UNIX_EPOCH,
                value.seconds_fragments() / 300,
            ))
        }),
        ColumnData::SmallDateTime(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_1900_TO_UNIX_EPOCH,
                u32::from(value.seconds_fragments()) * 60,
            ))
        }),
        ColumnData::Date(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH,
                0,
            ))
        }),
        ColumnData::Time(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(0, mssql_time_seconds(value)))
        }),
        ColumnData::DateTime2(value) => nullable(*value, mssql_datetime2),
        ColumnData::DateTimeOffset(value) => {
            nullable(*value, |value| mssql_datetime2(value.datetime2()))
        }
    }
}

fn mssql_time_seconds(time: tiberius::time::Time) -> u32 {
    let divisor = 10_u64.pow(u32::from(time.scale()));
    u32::try_from(time.increments() / divisor.max(1)).unwrap_or(0)
}

fn mssql_datetime2(value: tiberius::time::DateTime2) -> Cell {
    Cell::DateTime(DateTimeParts::from_unix_days(
        i64::from(value.date().days()) - DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH,
        mssql_time_seconds(value.time()),
    ))
}

struct MsSqlMetadataSource<'session> {
    session: &'session mut MsSqlSession,
}

impl<'session> MsSqlMetadataSource<'session> {
    fn new(session: &'session mut MsSqlSession) -> Self {
        Self { session }
    }
}

impl MetadataSource for MsSqlMetadataSource<'_> {
    async fn begin_readonly(&mut self) -> Result<(), CliError> {
        self.session.verify_readonly().await?;
        self.session.execute_batch("BEGIN TRANSACTION").await
    }

    async fn detect_layout(&mut self) -> Result<StorageLayout, CliError> {
        let rows = mssql_rows(
            self.session.client_mut()?,
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
    ) -> Result<open_sdbl::metadata::DbNames, CliError> {
        let rows = mssql_rows(
            self.session.client_mut()?,
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
            .collect::<Result<Vec<_>, CliError>>()?;
        let data = assemble_single_resource("DBNames", parts)?;
        run_metadata_blocking("DBNames", move || {
            parse_db_names(&data).map_err(CliError::from)
        })
        .await
    }

    async fn read_config(
        &mut self,
        layout: &StorageLayout,
        progress: &mut MetadataProgress,
    ) -> Result<ConfigMetadata, CliError> {
        let client = self.session.client_mut()?;
        let totals = mssql_rows(
            client,
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

        let rows = query_timeout("MSSQL Config query", async {
            client
                .simple_query(MsSqlMetadataQueries::config(layout))
                .await
                .map_err(CliError::mssql_query)
        })
        .await?
        .into_row_stream();
        let parts = rows.map(|row| {
            let row = row.map_err(CliError::mssql_query)?;
            Ok((
                required_mssql_string(&row, 0, "Config file name")?,
                required_mssql_i32(&row, 1, "Config part number")?,
                required_mssql_bytes(&row, 2, "Config payload")?,
            ))
        });
        decode_config_stream(
            assemble_parts(parts),
            CONFIG_DECODE_BATCH_SIZE,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
        )
        .await
    }

    async fn read_extension_resources(
        &mut self,
        layout: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, CliError> {
        let Some(query) = MsSqlMetadataQueries::extension_resources(layout) else {
            return Ok(Vec::new());
        };
        let rows = query_timeout("MSSQL ConfigCAS query", async {
            self.session
                .client_mut()?
                .simple_query(query)
                .await
                .map_err(CliError::mssql_query)
        })
        .await?
        .into_row_stream();
        let parts = rows.map(|row| {
            let row = row.map_err(CliError::mssql_query)?;
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
    ) -> Result<Vec<Vec<u8>>, CliError> {
        let Some(query) = MsSqlMetadataQueries::extension_restructure(layout) else {
            return Ok(Vec::new());
        };
        let rows = mssql_rows(
            self.session.client_mut()?,
            "MSSQL extension restructure query",
            query,
        )
        .await?;
        rows.iter()
            .map(|row| required_mssql_bytes(row, 0, "extension restructure payload"))
            .collect()
    }

    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, CliError> {
        let rows = mssql_rows(
            self.session.client_mut()?,
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
            parse_schema_storage(&data).map_err(CliError::from)
        })
        .await
    }

    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, CliError> {
        let rows = mssql_rows(
            self.session.client_mut()?,
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

    async fn commit_readonly(&mut self) -> Result<(), CliError> {
        match self.session.execute_batch("COMMIT TRANSACTION").await {
            Ok(()) => Ok(()),
            Err(error) => Err(self.session.rollback_after_error(error).await),
        }
    }

    async fn rollback_readonly(&mut self, original: CliError) -> CliError {
        self.session.rollback_after_error(original).await
    }
}

async fn mssql_rows(
    client: &mut MsSqlClient<MsSqlTransport>,
    label: &str,
    sql: &str,
) -> Result<Vec<tiberius::Row>, CliError> {
    query_timeout(label, async {
        client
            .simple_query(sql)
            .await
            .map_err(CliError::mssql_query)?
            .into_first_result()
            .await
            .map_err(CliError::mssql_query)
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

fn exactly_one_mssql_row<'rows>(
    rows: &'rows [tiberius::Row],
    name: &str,
) -> Result<&'rows tiberius::Row, CliError> {
    match rows {
        [row] => Ok(row),
        [] => Err(CliError::Data(format!("{name} resource is missing"))),
        _ => Err(CliError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

fn required_mssql_string(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<String, CliError> {
    row.try_get::<&str, _>(index)
        .map(|value| value.map(str::to_owned))
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_bytes(
    row: &tiberius::Row,
    index: usize,
    name: &str,
) -> Result<Vec<u8>, CliError> {
    row.try_get::<&[u8], _>(index)
        .map(|value| value.map(<[u8]>::to_vec))
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_i64(row: &tiberius::Row, index: usize, name: &str) -> Result<i64, CliError> {
    row.try_get::<i64, _>(index)
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

fn required_mssql_i32(row: &tiberius::Row, index: usize, name: &str) -> Result<i32, CliError> {
    row.try_get::<i32, _>(index)
        .map_err(CliError::mssql_query)?
        .ok_or_else(|| CliError::Data(format!("MSSQL returned NULL for {name}")))
}

impl MsSqlSession {
    pub(crate) const fn backend(&self) -> MsSqlBackend {
        self.backend
    }

    pub(crate) const fn is_dead(&self) -> bool {
        self.poisoned
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{decode_mssql_cell, should_disconnect_after_mssql_error};
    use crate::cells::{Cell, DateTimeParts};
    use crate::error::CliError;
    use tiberius::ColumnData;
    use tiberius::numeric::Numeric;
    use tiberius::time::{Date, DateTime, DateTime2, SmallDateTime, Time};

    #[test]
    fn decodes_tds_values_into_typed_cells() {
        assert_eq!(
            decode_mssql_cell(&ColumnData::Binary(Some(vec![0, 0x7d, 0xd6].into()))),
            Cell::Bytes(vec![0, 0x7d, 0xd6])
        );
        assert_eq!(
            decode_mssql_cell(&ColumnData::Numeric(Some(Numeric::new_with_scale(1550, 2)))),
            Cell::Number("15.50".to_owned())
        );
        assert_eq!(
            decode_mssql_cell(&ColumnData::Numeric(Some(Numeric::new_with_scale(15, 0)))),
            Cell::Number("15".to_owned())
        );
        assert_eq!(
            decode_mssql_cell(&ColumnData::Bit(Some(true))),
            Cell::Bool(true)
        );
        assert_eq!(decode_mssql_cell(&ColumnData::I32(None)), Cell::Null);
        let expected = DateTimeParts {
            year: 2024,
            month: 2,
            day: 29,
            hour: 12,
            minute: 34,
            second: 56,
        };
        // 2024-02-29 is 738_944 days after 0001-01-01 and 45_349 days after 1900-01-01.
        assert_eq!(
            decode_mssql_cell(&ColumnData::DateTime2(Some(DateTime2::new(
                Date::new(738_944),
                Time::new(452_961_234_567, 7),
            )))),
            Cell::DateTime(expected)
        );
        assert_eq!(
            decode_mssql_cell(&ColumnData::DateTime(Some(DateTime::new(
                45_349,
                45_296 * 300 + 150
            )))),
            Cell::DateTime(expected)
        );
        assert_eq!(
            decode_mssql_cell(&ColumnData::SmallDateTime(Some(SmallDateTime::new(
                45_349,
                12 * 60 + 34
            )))),
            Cell::DateTime(DateTimeParts {
                second: 0,
                ..expected
            })
        );
    }

    #[test]
    fn timeout_requires_dropping_mssql_without_rollback() {
        let timeout = CliError::DatabaseTimeout {
            operation: "MSSQL user query".to_owned(),
            duration: Duration::from_secs(120),
        };
        assert!(should_disconnect_after_mssql_error(&timeout));

        let semantic_error = CliError::Data("invalid row".to_owned());
        assert!(!should_disconnect_after_mssql_error(&semantic_error));
    }
}
