use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, MetadataSnapshot, MsSqlMetadataQueries, parse_db_names, parse_schema_storage,
};
use open_sdbl::query::MsSqlBackend;
use tiberius::{
    AuthMethod, Client as MsSqlClient, ColumnType as MsSqlColumnType, Config as MsSqlConfig,
};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use zeroize::Zeroizing;

use crate::args::MsSqlConnection;
use crate::auth::pgpass::Credentials;
use crate::error::CliError;
use crate::net::socks5::{connect_socks5, socks5_password};
use crate::pipeline::{
    ConfigDecodeLimits, ConfigMetadata, ConfigResource, MetadataSource, acquire_metadata,
    config_pipeline_depth, decode_catalog_values, decode_config_stream, run_metadata_blocking,
    unsigned_progress_total,
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
        let year_offset = session.read_year_offset().await?;
        session.backend =
            MsSqlBackend::new(year_offset).map_err(|error| CliError::Data(error.to_string()))?;
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
                .map(|row| {
                    (0..column_count)
                        .map(|index| mssql_cell_text(row, index))
                        .collect()
                })
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

fn mssql_cell_text(row: &tiberius::Row, index: usize) -> Result<Option<String>, CliError> {
    let column = row.columns().get(index).ok_or_else(|| {
        CliError::Data(format!(
            "MSSQL returned {} columns, but column {index} was requested",
            row.columns().len()
        ))
    })?;
    match column.column_type() {
        MsSqlColumnType::BigVarBin | MsSqlColumnType::BigBinary | MsSqlColumnType::Image => row
            .try_get::<&[u8], _>(index)
            .map(|value| value.map(format_mssql_binary))
            .map_err(CliError::mssql_query),
        _ => row
            .try_get::<&str, _>(index)
            .map(|value| value.map(str::to_owned))
            .map_err(CliError::mssql_query),
    }
}

pub(crate) fn format_mssql_binary(value: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(2 + value.len() * 2);
    output.push_str("0x");
    for byte in value {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
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

    async fn read_db_names(&mut self) -> Result<open_sdbl::metadata::DbNames, CliError> {
        let rows = mssql_rows(
            self.session.client_mut()?,
            "MSSQL DBNames query",
            MsSqlMetadataQueries::DB_NAMES,
        )
        .await?;
        let data = required_mssql_bytes(
            exactly_one_mssql_row(&rows, "DBNames")?,
            0,
            "DBNames payload",
        )?;
        run_metadata_blocking("DBNames", move || {
            parse_db_names(&data).map_err(CliError::from)
        })
        .await
    }

    async fn read_config(
        &mut self,
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
                .simple_query(MsSqlMetadataQueries::CONFIG)
                .await
                .map_err(CliError::mssql_query)
        })
        .await?
        .into_row_stream();
        let resources = rows.map(|row| {
            let row = row.map_err(CliError::mssql_query)?;
            Ok(ConfigResource {
                file_name: required_mssql_string(&row, 0, "Config file name")?,
                compressed: required_mssql_bytes(&row, 1, "Config payload")?,
            })
        });
        decode_config_stream(
            resources,
            CONFIG_DECODE_BATCH_SIZE,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
        )
        .await
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

    use super::{format_mssql_binary, should_disconnect_after_mssql_error};
    use crate::error::CliError;

    #[test]
    fn formats_binary_as_tsql_hex() {
        assert_eq!(format_mssql_binary(&[0, 0x7d, 0xd6]), "0x007DD6");
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
