//! Opening and driving a SQL Server session: TLS, transactions,
//! queries and recovery after a failure.

use open_sdbl::metadata::{MetadataSnapshot, MsSqlMetadataQueries, StorageLayout};
use open_sdbl::query::{MsSqlBackend, MsSqlDialectLevel};
use tiberius::{AuthMethod, Client as MsSqlClient, Config as MsSqlConfig};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use zeroize::Zeroizing;

use crate::args::MsSqlConnection;
use crate::auth::pgpass::Credentials;
use crate::cells::QueryRows;
use crate::error::CliError;
use crate::limits::{CONNECTION_TIMEOUT, MSSQL_TRANSACTION_COUNT, QUERY_TIMEOUT};
use crate::net::socks5::{connect_socks5, socks5_password};
#[cfg(test)]
use crate::pipeline::MetadataSource;
use crate::pipeline::acquire_metadata;
use crate::session::query_timeout;

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

pub(crate) struct MsSqlSession {
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

    pub(super) async fn connect_with_secrets(
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
            layout: None,
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

    pub(super) fn client_mut(&mut self) -> Result<&mut MsSqlClient<MsSqlTransport>, CliError> {
        self.client.as_mut().ok_or_else(|| {
            CliError::Database("MSSQL connection is closed; reconnect required".to_owned())
        })
    }

    pub(super) async fn execute_batch(&mut self, sql: &str) -> Result<(), CliError> {
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

    pub(super) async fn verify_database(&mut self) -> Result<(), CliError> {
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

    pub(super) async fn read_product_version(&mut self) -> Result<String, CliError> {
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

    pub(super) async fn read_year_offset(&mut self) -> Result<i32, CliError> {
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

    pub(super) fn ensure_usable(&self) -> Result<(), CliError> {
        if self.poisoned || self.client.is_none() {
            Err(CliError::Database(
                "MSSQL session is poisoned and cannot be reused; reconnect required".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    pub(super) async fn transaction_count(&mut self) -> Result<i32, CliError> {
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

    pub(super) async fn rollback_after_error(&mut self, original: CliError) -> CliError {
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

    pub(super) fn poison_and_drop(&mut self) {
        self.poisoned = true;
        drop(self.client.take());
    }

    pub(crate) async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        let (snapshot, layout) = acquire_metadata(&mut MsSqlMetadataSource::new(self)).await?;
        self.layout = Some(layout);
        Ok(snapshot)
    }

    /// The storage layout of the base, known after a metadata read.
    pub(crate) const fn layout(&self) -> Option<StorageLayout> {
        self.layout
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

#[cfg(test)]
#[path = "../../tests/mssql_session.rs"]
mod tests;
