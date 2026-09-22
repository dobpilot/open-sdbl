//! Opening and driving a SQL Server session: TLS, transactions,
//! queries and recovery after a failure.

use futures_util::StreamExt as _;
use open_sdbl::metadata::{
    MetadataSnapshot, MsSqlMetadataQueries, ResolutionReport, StorageLayout,
};
use open_sdbl::query::{MsSqlBackend, MsSqlDialectLevel};
use tiberius::{AuthMethod, Client as MsSqlClient, Config as MsSqlConfig};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use zeroize::Zeroizing;

use crate::cells::{Cell, QueryRows, RowFlow};
use crate::connection::MsSqlConnection;
use crate::credentials::Credentials;
use crate::error::DbError;
use crate::limits::Limits;
use crate::net::socks5::{connect_socks5, socks5_password};
#[cfg(test)]
use crate::pipeline::MetadataSource;
use crate::pipeline::{AcquiredConfiguration, acquire_configuration, acquire_metadata};
use crate::progress::MetadataProgress;
use crate::rows::{ReadEnd, drive_rows};
use crate::session::query_timeout;

/// The statement that reads the transaction depth of an MSSQL session.
const MSSQL_TRANSACTION_COUNT: &str = "SELECT CONVERT(int, @@TRANCOUNT)";

type MsSqlTransport = Compat<TcpStream>;
use super::cells::{mssql_row, should_disconnect_after_mssql_error};
use super::metadata::{
    MsSqlMetadataSource, apply_mssql_cleanup, exactly_one_mssql_row, mssql_rows,
    required_mssql_i32, required_mssql_string,
};

#[derive(Clone)]
pub(super) struct MsSqlSecrets {
    pub(super) password: Zeroizing<String>,
    pub(super) socks5_password: Option<Zeroizing<String>>,
}

/// One open Microsoft SQL Server session.
pub struct MsSqlSession {
    pub(super) client: Option<MsSqlClient<MsSqlTransport>>,
    pub(super) connection: MsSqlConnection,
    pub(super) database: String,
    pub(super) backend: MsSqlBackend,
    /// `SERVERPROPERTY('ProductVersion')` of the connected server.
    pub(super) product_version: String,
    pub(super) poisoned: bool,
    pub(super) secrets: MsSqlSecrets,
    /// The storage layout the last metadata read detected.
    pub(super) layout: Option<StorageLayout>,
    pub(super) limits: Limits,
}

impl MsSqlSession {
    /// Opens a session against the described base under `limits`.
    pub async fn connect(
        connection: &MsSqlConnection,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        let secrets = MsSqlSecrets {
            password: credentials.mssql.required("MSSQL_PASSWORD")?,
            socks5_password: socks5_password(
                connection.options.socks5_proxy.as_ref(),
                &credentials.socks5,
            )?,
        };
        Self::connect_with_secrets(connection, secrets, limits).await
    }

    /// The limits this session applies to everything it does.
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    pub(super) async fn connect_with_secrets(
        connection: &MsSqlConnection,
        secrets: MsSqlSecrets,
        limits: Limits,
    ) -> Result<Self, DbError> {
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
                limits.connection_timeout,
            )
            .await?
        } else {
            timeout(
                limits.connection_timeout,
                TcpStream::connect((options.host.as_str(), options.port)),
            )
            .await
            .map_err(|_| {
                DbError::Database(format!(
                    "MSSQL TCP connection timed out after {} seconds",
                    limits.connection_timeout.as_secs()
                ))
            })?
            .map_err(|error| {
                DbError::Io("cannot connect to MSSQL TCP endpoint".to_owned(), error)
            })?
        };
        stream.set_nodelay(true).map_err(|error| {
            DbError::Io("cannot configure MSSQL TCP connection".to_owned(), error)
        })?;
        let client = timeout(
            limits.connection_timeout,
            MsSqlClient::connect(configuration, stream.compat_write()),
        )
        .await
        .map_err(|_| {
            DbError::Database(format!(
                "MSSQL startup timed out after {} seconds",
                limits.connection_timeout.as_secs()
            ))
        })?
        .map_err(DbError::mssql_connection)?;
        let mut session = Self {
            client: Some(client),
            connection: connection.clone(),
            database: options.database.clone(),
            backend: MsSqlBackend::default(),
            product_version: String::new(),
            poisoned: false,
            secrets,
            layout: None,
            limits,
        };
        session
            .execute_batch(
                &format!(
                    "SET QUOTED_IDENTIFIER ON; SET TRANSACTION ISOLATION LEVEL READ COMMITTED; SET LOCK_TIMEOUT {};",
                    limits.query_timeout.as_millis()
                ),
            )
            .await?;
        session.verify_database().await?;
        session.product_version = session.read_product_version().await?;
        let dialect_level = match connection.dialect_level {
            Some(level) => level,
            None => MsSqlDialectLevel::from_product_version(&session.product_version).ok_or_else(
                || {
                    DbError::Data(format!(
                        "unsupported SQL Server product version {:?}; pass --mssql-dialect explicitly",
                        session.product_version
                    ))
                },
            )?,
        };
        let year_offset = session.read_year_offset().await?;
        session.backend = MsSqlBackend::new(year_offset)
            .map_err(|error| DbError::Data(error.to_string()))?
            .with_dialect_level(dialect_level);
        Ok(session)
    }

    pub(super) fn client_mut(&mut self) -> Result<&mut MsSqlClient<MsSqlTransport>, DbError> {
        self.client.as_mut().ok_or_else(|| {
            DbError::Database("MSSQL connection is closed; reconnect required".to_owned())
        })
    }

    pub(super) async fn execute_batch(&mut self, sql: &str) -> Result<(), DbError> {
        self.ensure_usable()?;
        let limits = self.limits;
        let result = {
            let client = self.client_mut()?;
            query_timeout(limits, "MSSQL batch", async {
                client
                    .simple_query(sql)
                    .await
                    .map_err(DbError::mssql_query)?
                    .into_results()
                    .await
                    .map_err(DbError::mssql_query)?;
                Ok(())
            })
            .await
        };
        if result
            .as_ref()
            .is_err_and(DbError::requires_mssql_disconnect)
        {
            self.poison_and_drop();
        }
        result
    }

    pub(super) async fn verify_database(&mut self) -> Result<(), DbError> {
        let limits = self.limits;
        let rows = mssql_rows(
            self.client_mut()?,
            limits,
            "MSSQL database verification",
            MsSqlMetadataQueries::VERIFY_DATABASE,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "database verification")?;
        let actual = required_mssql_string(row, 0, "database name")?;
        let status = required_mssql_string(row, 1, "database status")?;
        if !actual.eq_ignore_ascii_case(&self.database) || !status.eq_ignore_ascii_case("ONLINE") {
            return Err(DbError::Data(format!(
                "unexpected MSSQL database state: database={actual:?}, status={status:?}"
            )));
        }
        Ok(())
    }

    pub(super) async fn read_product_version(&mut self) -> Result<String, DbError> {
        let limits = self.limits;
        let rows = mssql_rows(
            self.client_mut()?,
            limits,
            "MSSQL product version query",
            MsSqlMetadataQueries::PRODUCT_VERSION,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "product version")?;
        required_mssql_string(row, 0, "SQL Server product version")
    }

    /// One-line description of the connected server and the dialect level
    /// generated T-SQL targets.
    pub fn server_description(&self) -> String {
        format!(
            "MSSQL dialect: {} (server {})",
            self.backend.dialect_level(),
            self.product_version
        )
    }

    pub(super) async fn read_year_offset(&mut self) -> Result<i32, DbError> {
        let limits = self.limits;
        let rows = mssql_rows(
            self.client_mut()?,
            limits,
            "MSSQL _YearOffset query",
            MsSqlMetadataQueries::YEAR_OFFSET,
        )
        .await?;
        let row = exactly_one_mssql_row(&rows, "_YearOffset")?;
        let offset = row
            .try_get::<i32, _>(0)
            .map_err(DbError::mssql_query)?
            .ok_or_else(|| {
                DbError::Data("MSSQL returned NULL for _YearOffset.Offset".to_owned())
            })?;
        if !matches!(offset, 0 | 2000) {
            return Err(DbError::Data(format!(
                "unsupported MSSQL _YearOffset.Offset value {offset}; expected 0 or 2000"
            )));
        }
        Ok(offset)
    }

    pub(super) fn ensure_usable(&self) -> Result<(), DbError> {
        if self.poisoned || self.client.is_none() {
            Err(DbError::Database(
                "MSSQL session is poisoned and cannot be reused; reconnect required".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    pub(super) async fn transaction_count(&mut self) -> Result<i32, DbError> {
        let limits = self.limits;
        let rows = mssql_rows(
            self.client_mut()?,
            limits,
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

    pub(super) async fn rollback_after_error(&mut self, original: DbError) -> DbError {
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
            Err(cleanup) => DbError::Database(format!(
                "{original}; MSSQL rollback cleanup failed and the session was poisoned: {cleanup}"
            )),
        }
    }

    pub(super) fn poison_and_drop(&mut self) {
        self.poisoned = true;
        drop(self.client.take());
    }

    /// Reads and resolves the metadata of the base, answering the snapshot
    /// and the report of what the resolver had to recover from.
    pub async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        let (snapshot, layout, report) =
            acquire_metadata(&mut MsSqlMetadataSource::new(self), progress).await?;
        self.layout = Some(layout);
        Ok((snapshot, report))
    }

    /// Reads the whole configuration of the base in one read-only
    /// transaction: the metadata [`MsSqlSession::metadata`] answers,
    /// every resource of `Config`, and the resources of each extension.
    pub async fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        let acquired = acquire_configuration(&mut MsSqlMetadataSource::new(self), progress).await?;
        self.layout = Some(acquired.layout);
        Ok(acquired)
    }

    /// The storage layout of the base, known after a metadata read.
    pub const fn layout(&self) -> Option<StorageLayout> {
        self.layout
    }

    /// Probes the service-table layout without starting a transaction; used
    /// by diagnostics and live tests.
    #[cfg(test)]
    pub(crate) async fn storage_layout(&mut self) -> Result<StorageLayout, DbError> {
        MsSqlMetadataSource::new(self).detect_layout().await
    }

    /// Runs one read-only statement and decodes `column_count` columns.
    pub async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        let mut rows = QueryRows::new();
        self.query_each(sql, column_count, |row| {
            rows.push(row);
            Ok(RowFlow::Continue)
        })
        .await?;
        Ok(rows)
    }

    /// Reads one read-only statement row by row.
    ///
    /// Each row is decoded on its own and handed to `on_row` before the
    /// next is read from the server. Answering [`RowFlow::Stop`] ends the
    /// read there: the rows that follow are never fetched or decoded.
    ///
    /// Stopping early **costs the connection**. This crate gives SQL
    /// Server no execution limit — `SET LOCK_TIMEOUT` bounds lock waiting
    /// only — so dropping the statement is the only way to end it on the
    /// server, and a TDS stream abandoned between protocol messages cannot
    /// be reused. The session is therefore poisoned, reports
    /// [`MsSqlSession::is_dead`], and the caller reconnects with
    /// [`MsSqlSession::cancel_and_reconnect`].
    ///
    /// # Errors
    ///
    /// Returns what the server reported, what decoding a row reported, or
    /// what `on_row` returned.
    pub async fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        self.execute_batch("BEGIN TRANSACTION").await?;
        let limits = self.limits;
        let client = self.client_mut()?;
        let result = query_timeout(limits, "MSSQL user query", async {
            let rows = client
                .simple_query(sql)
                .await
                .map_err(DbError::mssql_query)?
                .into_row_stream();
            let decoded =
                rows.map(move |row| mssql_row(&row.map_err(DbError::mssql_query)?, column_count));
            drive_rows(decoded, on_row).await
        })
        .await;
        match result {
            // The statement ran out: the stream is between messages only
            // if it was abandoned, so a finished read commits as before.
            Ok(ReadEnd::Exhausted) => match self.execute_batch("COMMIT TRANSACTION").await {
                Ok(()) => Ok(()),
                Err(error) => Err(self.rollback_after_error(error).await),
            },
            // The caller stopped: the statement is still producing rows on
            // the server, and the only way to end it is to drop what
            // carries it.
            Ok(ReadEnd::Stopped) => {
                self.poison_and_drop();
                Ok(())
            }
            Err(error) => Err(self.rollback_after_error(error).await),
        }
    }

    /// Drops the session and opens a new one, the only way SQL Server
    /// cancellation is safe here.
    pub async fn cancel_and_reconnect(&mut self) -> Result<(), DbError> {
        // Dropping an in-flight Tiberius future can leave the TDS stream between
        // protocol messages. Do not send cleanup commands on that connection.
        self.poison_and_drop();
        let connection = self.connection.clone();
        let replacement =
            Self::connect_with_secrets(&connection, self.secrets.clone(), self.limits).await?;
        *self = replacement;
        Ok(())
    }

    /// Closes the session.
    pub async fn close(self) -> Result<(), DbError> {
        drop(self.client);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/mssql_session.rs"]
mod tests;
