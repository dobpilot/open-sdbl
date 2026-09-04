use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, MetadataSnapshot, PostgresMetadataQueries, parse_db_names, parse_schema_storage,
};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{WebPkiServerVerifier, verify_server_cert_signed_by_trust_anchor};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{ClientConfig as RustlsClientConfig, DigitallySignedStruct, RootCertStore};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_postgres::config::SslMode;
use tokio_postgres::tls::MakeTlsConnect;
use tokio_postgres::types::ToSql;
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};
use tokio_postgres_rustls::MakeRustlsConnect;
use zeroize::Zeroizing;

use crate::args::{PostgresConnection, PostgresSslMode};
use crate::auth::pgpass::{Credentials, postgres_password};
use crate::error::CliError;
use crate::net::socks5::{connect_socks5, socks5_password};
use crate::pipeline::{
    ConfigDecodeLimits, ConfigMetadata, ConfigResource, MetadataSource, acquire_metadata,
    config_pipeline_depth, decode_catalog_values, decode_config_stream, run_metadata_blocking,
    unsigned_progress_total,
};
use crate::progress::MetadataProgress;
use crate::{
    CONFIG_DECODE_BATCH_SIZE, CONNECTION_TIMEOUT, POSTGRES_CLOSE_TIMEOUT, QUERY_TIMEOUT, QueryRows,
    query_timeout,
};

#[derive(Debug)]
struct PostgresServerCertVerifier {
    certificate_roots: Option<Arc<RootCertStore>>,
    signature_verifier: Arc<WebPkiServerVerifier>,
}

impl ServerCertVerifier for PostgresServerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if let Some(roots) = &self.certificate_roots {
            let certificate = ParsedCertificate::try_from(end_entity)?;
            let provider = rustls::crypto::ring::default_provider();
            verify_server_cert_signed_by_trust_anchor(
                &certificate,
                roots,
                intermediates,
                now,
                provider.signature_verification_algorithms.all,
            )?;
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.signature_verifier
            .verify_tls12_signature(message, certificate, signature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.signature_verifier
            .verify_tls13_signature(message, certificate, signature)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.signature_verifier.supported_verify_schemes()
    }
}

fn postgres_tls_connector(mode: PostgresSslMode) -> Result<MakeRustlsConnect, CliError> {
    let signature_roots = Arc::new(RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    });
    let signature_verifier = WebPkiServerVerifier::builder(Arc::clone(&signature_roots))
        .build()
        .map_err(|error| CliError::Data(format!("cannot configure PostgreSQL TLS: {error}")))?;

    let certificate_roots = match mode {
        PostgresSslMode::Require => None,
        PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull => {
            let native_certificates = rustls_native_certs::load_native_certs();
            let mut roots = RootCertStore::empty();
            let (accepted, _) = roots.add_parsable_certificates(native_certificates.certs);
            if accepted == 0 {
                let details = native_certificates
                    .errors
                    .first()
                    .map_or_else(String::new, |error| format!(": {error}"));
                return Err(CliError::Data(format!(
                    "cannot load any native CA certificates for PostgreSQL TLS{details}"
                )));
            }
            Some(Arc::new(roots))
        }
        PostgresSslMode::Disable => {
            return Err(CliError::Data(
                "internal error: TLS connector requested for plaintext PostgreSQL".to_owned(),
            ));
        }
    };
    let roots = certificate_roots
        .as_deref()
        .cloned()
        .unwrap_or_else(|| (*signature_roots).clone());
    let mut configuration = RustlsClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    match mode {
        PostgresSslMode::Require => configuration.dangerous().set_certificate_verifier(Arc::new(
            PostgresServerCertVerifier {
                certificate_roots: None,
                signature_verifier,
            },
        )),
        PostgresSslMode::VerifyCa => configuration.dangerous().set_certificate_verifier(Arc::new(
            PostgresServerCertVerifier {
                certificate_roots,
                signature_verifier,
            },
        )),
        PostgresSslMode::VerifyFull => {}
        PostgresSslMode::Disable => unreachable!("handled before TLS configuration"),
    }
    Ok(MakeRustlsConnect::new(configuration))
}

pub(crate) struct PostgresSession {
    client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    connection: PostgresConnection,
    socks5_password: Option<Zeroizing<String>>,
}

impl PostgresSession {
    pub(crate) async fn connect(
        connection: &PostgresConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        let mut configuration = tokio_postgres::Config::new();
        configuration
            .host(&connection.host)
            .port(connection.port)
            .dbname(&connection.database)
            .user(&connection.user)
            .connect_timeout(CONNECTION_TIMEOUT)
            .options(format!(
                "-c statement_timeout={}",
                QUERY_TIMEOUT.as_millis()
            ))
            .ssl_mode(match connection.sslmode {
                PostgresSslMode::Disable => SslMode::Disable,
                PostgresSslMode::Require
                | PostgresSslMode::VerifyCa
                | PostgresSslMode::VerifyFull => SslMode::Require,
            });
        if let Some(password) = postgres_password(connection, &credentials.postgres)? {
            configuration.password(password.as_str());
        }
        let socks5_password =
            socks5_password(connection.socks5_proxy.as_ref(), &credentials.socks5)?;

        let (client, driver) = match (connection.sslmode, &connection.socks5_proxy) {
            (PostgresSslMode::Disable, Some(proxy)) => {
                let stream = connect_socks5(
                    proxy,
                    socks5_password.as_deref().map(String::as_str),
                    &connection.host,
                    connection.port,
                )
                .await?;
                connect_postgres_raw(&configuration, stream, CONNECTION_TIMEOUT).await?
            }
            (PostgresSslMode::Disable, None) => {
                let (client, connection_driver) = configuration
                    .connect(NoTls)
                    .await
                    .map_err(CliError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
            (mode, Some(proxy)) => {
                let stream = connect_socks5(
                    proxy,
                    socks5_password.as_deref().map(String::as_str),
                    &connection.host,
                    connection.port,
                )
                .await?;
                connect_postgres_raw_tls(
                    &configuration,
                    stream,
                    postgres_tls_connector(mode)?,
                    &connection.host,
                    CONNECTION_TIMEOUT,
                )
                .await?
            }
            (mode, None) => {
                let (client, connection_driver) = configuration
                    .connect(postgres_tls_connector(mode)?)
                    .await
                    .map_err(CliError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
        };
        Ok(Self {
            client,
            driver,
            connection: connection.clone(),
            socks5_password,
        })
    }

    pub(crate) async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        let transaction = query_timeout("PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(CliError::from)
        })
        .await?;
        acquire_metadata(&mut PostgresMetadataSource::new(transaction)).await
    }

    pub(crate) async fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> Result<QueryRows, CliError> {
        let transaction = query_timeout("PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(CliError::from)
        })
        .await?;
        if let Err(error) = verify_transaction(&transaction).await {
            let _ = query_timeout("PostgreSQL transaction rollback", async {
                transaction.rollback().await.map_err(CliError::from)
            })
            .await;
            return Err(error);
        }
        let query = query_timeout("PostgreSQL query", async {
            transaction.query(sql, &[]).await.map_err(CliError::from)
        })
        .await;
        match query {
            Ok(rows) => {
                let rows = rows
                    .iter()
                    .map(|row| {
                        (0..column_count)
                            .map(|index| row.try_get(index).map_err(CliError::from))
                            .collect()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                query_timeout("PostgreSQL transaction commit", async {
                    transaction.commit().await.map_err(CliError::from)
                })
                .await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = query_timeout("PostgreSQL transaction rollback", async {
                    transaction.rollback().await.map_err(CliError::from)
                })
                .await;
                Err(error)
            }
        }
    }

    pub(crate) async fn cancel_query(
        &self,
        token: tokio_postgres::CancelToken,
    ) -> Result<(), CliError> {
        let connection = &self.connection;
        query_timeout("PostgreSQL query cancellation", async {
            match (connection.sslmode, &connection.socks5_proxy) {
                (PostgresSslMode::Disable, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                    )
                    .await?;
                    token
                        .cancel_query_raw(stream, NoTls)
                        .await
                        .map_err(CliError::database_connection)
                }
                (PostgresSslMode::Disable, None) => token
                    .cancel_query(NoTls)
                    .await
                    .map_err(CliError::database_connection),
                (mode, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                    )
                    .await?;
                    let mut tls = postgres_tls_connector(mode)?;
                    let tls = <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(
                        &mut tls,
                        &connection.host,
                    )
                    .map_err(|error| {
                        CliError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
                    })?;
                    token
                        .cancel_query_raw(stream, tls)
                        .await
                        .map_err(CliError::database_connection)
                }
                (mode, None) => token
                    .cancel_query(postgres_tls_connector(mode)?)
                    .await
                    .map_err(CliError::database_connection),
            }
        })
        .await
    }

    pub(crate) async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        await_postgres_driver(self.driver, POSTGRES_CLOSE_TIMEOUT).await
    }
}

pub(crate) async fn await_postgres_driver(
    mut driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    close_timeout: Duration,
) -> Result<(), CliError> {
    match timeout(close_timeout, &mut driver).await {
        Ok(result) => result
            .map_err(|error| {
                CliError::Database(format!("PostgreSQL connection task failed: {error}"))
            })?
            .map_err(CliError::database_connection),
        Err(_) => {
            driver.abort();
            let _ = driver.await;
            Err(CliError::Database(format!(
                "PostgreSQL connection close timed out after {:.3} seconds; driver aborted",
                close_timeout.as_secs_f64()
            )))
        }
    }
}

pub(crate) async fn connect_postgres_raw(
    configuration: &tokio_postgres::Config,
    stream: TcpStream,
    connect_timeout: Duration,
) -> Result<
    (
        tokio_postgres::Client,
        tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    ),
    CliError,
> {
    match timeout(connect_timeout, configuration.connect_raw(stream, NoTls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(CliError::database_connection(error)),
        Err(_) => Err(CliError::Database(format!(
            "PostgreSQL startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}

async fn connect_postgres_raw_tls(
    configuration: &tokio_postgres::Config,
    stream: TcpStream,
    mut tls: MakeRustlsConnect,
    hostname: &str,
    connect_timeout: Duration,
) -> Result<
    (
        tokio_postgres::Client,
        tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    ),
    CliError,
> {
    let tls =
        <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(&mut tls, hostname)
            .map_err(|error| {
                CliError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
            })?;
    match timeout(connect_timeout, configuration.connect_raw(stream, tls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(CliError::database_connection(error)),
        Err(_) => Err(CliError::Database(format!(
            "PostgreSQL TLS startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}

struct PostgresMetadataSource<'transaction> {
    transaction: Option<Transaction<'transaction>>,
}

impl<'transaction> PostgresMetadataSource<'transaction> {
    fn new(transaction: Transaction<'transaction>) -> Self {
        Self {
            transaction: Some(transaction),
        }
    }

    fn transaction(&self) -> Result<&Transaction<'transaction>, CliError> {
        self.transaction.as_ref().ok_or_else(|| {
            CliError::Database("PostgreSQL metadata transaction is closed".to_owned())
        })
    }
}

impl MetadataSource for PostgresMetadataSource<'_> {
    async fn begin_readonly(&mut self) -> Result<(), CliError> {
        verify_transaction(self.transaction()?).await
    }

    async fn read_db_names(&mut self) -> Result<open_sdbl::metadata::DbNames, CliError> {
        let rows = postgres_rows(
            self.transaction()?,
            "PostgreSQL DBNames query",
            PostgresMetadataQueries::DB_NAMES,
        )
        .await?;
        let data: Vec<u8> = exactly_one_row(&rows, "DBNames")?.try_get(0)?;
        run_metadata_blocking("DBNames", move || {
            parse_db_names(&data).map_err(CliError::from)
        })
        .await
    }

    async fn read_config(
        &mut self,
        progress: &mut MetadataProgress,
    ) -> Result<ConfigMetadata, CliError> {
        let transaction = self.transaction()?;
        let totals = query_timeout("PostgreSQL Config totals query", async {
            transaction
                .query_one(PostgresMetadataQueries::CONFIG_TOTALS, &[])
                .await
                .map_err(CliError::from)
        })
        .await?;
        progress.config_totals(
            unsigned_progress_total(totals.try_get(0)?, "resource count")?,
            unsigned_progress_total(totals.try_get(1)?, "compressed byte count")?,
        );

        let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
        let rows = query_timeout("PostgreSQL Config query", async {
            transaction
                .query_raw(PostgresMetadataQueries::CONFIG, parameters)
                .await
                .map_err(CliError::from)
        })
        .await?;
        let resources = rows.map(|row| {
            let row = row?;
            Ok(ConfigResource {
                file_name: row.try_get(0)?,
                compressed: row.try_get(1)?,
            })
        });
        query_timeout(
            "PostgreSQL Config stream",
            decode_config_stream(
                resources,
                CONFIG_DECODE_BATCH_SIZE,
                config_pipeline_depth(),
                ConfigDecodeLimits::default(),
                progress,
            ),
        )
        .await
    }

    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, CliError> {
        let rows = postgres_rows(
            self.transaction()?,
            "PostgreSQL SchemaStorage query",
            PostgresMetadataQueries::SCHEMA,
        )
        .await?;
        let data: Vec<u8> = exactly_one_row(&rows, "SchemaStorage")?.try_get(0)?;
        run_metadata_blocking("SchemaStorage", move || {
            parse_schema_storage(&data).map_err(CliError::from)
        })
        .await
    }

    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, CliError> {
        let rows = postgres_rows(
            self.transaction()?,
            "PostgreSQL catalog query",
            PostgresMetadataQueries::CATALOG,
        )
        .await?;
        run_metadata_blocking("PostgreSQL catalog", move || decode_catalog_rows(rows)).await
    }

    async fn commit_readonly(&mut self) -> Result<(), CliError> {
        let transaction = self.transaction.take().ok_or_else(|| {
            CliError::Database("PostgreSQL metadata transaction is closed".to_owned())
        })?;
        query_timeout("PostgreSQL transaction commit", async {
            transaction.commit().await.map_err(CliError::from)
        })
        .await
    }

    async fn rollback_readonly(&mut self, original: CliError) -> CliError {
        if let Some(transaction) = self.transaction.take() {
            let _ = query_timeout("PostgreSQL transaction rollback", async {
                transaction.rollback().await.map_err(CliError::from)
            })
            .await;
        }
        original
    }
}

async fn verify_transaction(transaction: &Transaction<'_>) -> Result<(), CliError> {
    let transaction_mode = query_timeout("PostgreSQL read-only verification", async {
        transaction
            .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
            .await
            .map_err(CliError::from)
    })
    .await?;
    let read_only: String = transaction_mode.try_get(0)?;
    let isolation: String = transaction_mode.try_get(1)?;
    if read_only != "on" || !isolation.eq_ignore_ascii_case("read committed") {
        return Err(CliError::Data(format!(
            "unsafe PostgreSQL transaction mode: read_only={read_only:?}, isolation={isolation:?}"
        )));
    }
    Ok(())
}

async fn postgres_rows(
    transaction: &Transaction<'_>,
    label: &str,
    sql: &str,
) -> Result<Vec<Row>, CliError> {
    query_timeout(label, async {
        transaction.query(sql, &[]).await.map_err(CliError::from)
    })
    .await
}

fn exactly_one_row<'rows>(rows: &'rows [Row], name: &str) -> Result<&'rows Row, CliError> {
    match rows {
        [row] => Ok(row),
        [] => Err(CliError::Data(format!("{name} resource is missing"))),
        _ => Err(CliError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

fn decode_catalog_rows(rows: Vec<Row>) -> Result<Vec<LiveTable>, CliError> {
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::await_postgres_driver;

    #[tokio::test]
    async fn aborts_a_stalled_driver_on_close() {
        let driver = tokio::spawn(async {
            std::future::pending::<Result<(), tokio_postgres::Error>>().await
        });
        assert!(
            await_postgres_driver(driver, Duration::from_millis(5))
                .await
                .unwrap_err()
                .to_string()
                .contains("driver aborted")
        );
    }
}
