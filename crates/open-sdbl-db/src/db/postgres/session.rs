//! Opening and driving a PostgreSQL session: TLS, the read-only
//! transaction, queries and shutdown.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt as _;
use open_sdbl::metadata::{MetadataSnapshot, ResolutionReport};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{WebPkiServerVerifier, verify_server_cert_signed_by_trust_anchor};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{ClientConfig as RustlsClientConfig, DigitallySignedStruct, RootCertStore};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_postgres::config::SslMode;
use tokio_postgres::tls::MakeTlsConnect;
use tokio_postgres::types::ToSql;
use tokio_postgres::{IsolationLevel, NoTls};
use tokio_postgres_rustls::MakeRustlsConnect;
use zeroize::Zeroizing;

use super::cells::PostgresCell;
use super::metadata::{PostgresMetadataSource, verify_transaction};
use crate::cells::{Cell, QueryRows, RowFlow};
use crate::connection::{PostgresConnection, PostgresSslMode};
use crate::credentials::Credentials;
use crate::error::DbError;
use crate::limits::Limits;
use crate::net::socks5::{connect_socks5, socks5_password};
use crate::pipeline::acquire_metadata;
use crate::progress::MetadataProgress;
use crate::rows::drive_rows;
use crate::session::query_timeout;
use open_sdbl::metadata::StorageLayout;

#[cfg(test)]
#[path = "../../tests/postgres_session.rs"]
mod tests;

#[derive(Debug)]
pub(super) struct PostgresServerCertVerifier {
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

pub(super) fn postgres_tls_connector(
    mode: PostgresSslMode,
    trust_ca_file: Option<&str>,
) -> Result<MakeRustlsConnect, DbError> {
    let signature_roots = Arc::new(RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    });
    let signature_verifier = WebPkiServerVerifier::builder(Arc::clone(&signature_roots))
        .build()
        .map_err(|error| DbError::Data(format!("cannot configure PostgreSQL TLS: {error}")))?;

    let certificate_roots = match mode {
        PostgresSslMode::Require => None,
        PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull => {
            let mut roots = RootCertStore::empty();
            let (accepted, details) = if let Some(path) = trust_ca_file {
                let bytes = std::fs::read(path).map_err(|error| {
                    DbError::Io(
                        format!("cannot read PostgreSQL CA certificate file {path:?}"),
                        error,
                    )
                })?;
                let pem_certificates = CertificateDer::pem_slice_iter(&bytes)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        DbError::Data(format!(
                            "cannot parse PostgreSQL CA certificate file {path:?}: {error}"
                        ))
                    })?;
                let certificates = if pem_certificates.is_empty() {
                    vec![CertificateDer::from(bytes)]
                } else {
                    pem_certificates
                };
                let (accepted, rejected) = roots.add_parsable_certificates(certificates);
                (
                    accepted,
                    (rejected != 0).then(|| format!(": {rejected} certificate(s) were invalid")),
                )
            } else {
                let native_certificates = rustls_native_certs::load_native_certs();
                let (accepted, rejected) =
                    roots.add_parsable_certificates(native_certificates.certs);
                (
                    accepted,
                    native_certificates
                        .errors
                        .first()
                        .map(|error| format!(": {error}"))
                        .or_else(|| {
                            (rejected != 0)
                                .then(|| format!(": {rejected} certificate(s) were invalid"))
                        }),
                )
            };
            if accepted == 0 {
                return Err(DbError::Data(format!(
                    "cannot load any CA certificates for PostgreSQL TLS{}",
                    details.unwrap_or_default()
                )));
            }
            Some(Arc::new(roots))
        }
        PostgresSslMode::Disable => {
            return Err(DbError::Data(
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

/// One open PostgreSQL session.
pub struct PostgresSession {
    pub(super) client: tokio_postgres::Client,
    pub(super) driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    pub(super) connection: PostgresConnection,
    pub(super) socks5_password: Option<Zeroizing<String>>,
    /// The storage layout the last metadata read detected.
    pub(super) layout: Option<StorageLayout>,
    pub(super) limits: Limits,
}

impl PostgresSession {
    /// Opens a session against the described base under `limits`.
    pub async fn connect(
        connection: &PostgresConnection,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        let mut configuration = tokio_postgres::Config::new();
        configuration
            .host(&connection.host)
            .port(connection.port)
            .dbname(&connection.database)
            .user(&connection.user)
            .connect_timeout(limits.connection_timeout)
            .options(format!(
                "-c statement_timeout={}",
                limits.query_timeout.as_millis()
            ))
            .ssl_mode(match connection.sslmode {
                PostgresSslMode::Disable => SslMode::Disable,
                PostgresSslMode::Require
                | PostgresSslMode::VerifyCa
                | PostgresSslMode::VerifyFull => SslMode::Require,
            });
        // The password policy — a file, a vault, an environment variable —
        // is the caller's; this crate only uses the secret it was handed.
        if let Some(password) = credentials.postgres.optional("PGPASSWORD")? {
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
                    limits.connection_timeout,
                )
                .await?;
                connect_postgres_raw(&configuration, stream, limits.connection_timeout).await?
            }
            (PostgresSslMode::Disable, None) => {
                let (client, connection_driver) = configuration
                    .connect(NoTls)
                    .await
                    .map_err(DbError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
            (mode, Some(proxy)) => {
                let stream = connect_socks5(
                    proxy,
                    socks5_password.as_deref().map(String::as_str),
                    &connection.host,
                    connection.port,
                    limits.connection_timeout,
                )
                .await?;
                connect_postgres_raw_tls(
                    &configuration,
                    stream,
                    postgres_tls_connector(mode, connection.trust_ca_file.as_deref())?,
                    &connection.host,
                    limits.connection_timeout,
                )
                .await?
            }
            (mode, None) => {
                let (client, connection_driver) = configuration
                    .connect(postgres_tls_connector(
                        mode,
                        connection.trust_ca_file.as_deref(),
                    )?)
                    .await
                    .map_err(DbError::database_connection)?;
                (client, tokio::spawn(connection_driver))
            }
        };
        Ok(Self {
            client,
            driver,
            connection: connection.clone(),
            socks5_password,
            layout: None,
            limits,
        })
    }

    /// The limits this session applies to everything it does.
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Reads and resolves the metadata of the base, answering the snapshot
    /// and the report of what the resolver had to recover from.
    pub async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        let limits = self.limits;
        let transaction = query_timeout(limits, "PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(DbError::from)
        })
        .await?;
        let (snapshot, layout, report) = acquire_metadata(
            &mut PostgresMetadataSource::new(transaction, limits),
            progress,
        )
        .await?;
        self.layout = Some(layout);
        Ok((snapshot, report))
    }

    /// The storage layout of the base, known after a metadata read.
    pub const fn layout(&self) -> Option<StorageLayout> {
        self.layout
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
    /// read there: the rows that follow are never fetched or decoded. The
    /// session stays usable either way, because the server bounds the
    /// statement by the `statement_timeout` set at connect.
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
        let limits = self.limits;
        let transaction = query_timeout(limits, "PostgreSQL transaction start", async {
            self.client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .read_only(true)
                .start()
                .await
                .map_err(DbError::from)
        })
        .await?;
        if let Err(error) = verify_transaction(&transaction, limits).await {
            let _ = query_timeout(limits, "PostgreSQL transaction rollback", async {
                transaction.rollback().await.map_err(DbError::from)
            })
            .await;
            return Err(error);
        }
        let read = async {
            let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
            let rows = query_timeout(limits, "PostgreSQL query", async {
                transaction
                    .query_raw(sql, parameters)
                    .await
                    .map_err(DbError::from)
            })
            .await?;
            let decoded = rows.map(move |row| decode_postgres_row(row, column_count));
            drive_rows(decoded, on_row).await
        }
        .await;
        match read {
            Ok(_) => {
                query_timeout(limits, "PostgreSQL transaction commit", async {
                    transaction.commit().await.map_err(DbError::from)
                })
                .await
            }
            Err(error) => {
                let _ = query_timeout(limits, "PostgreSQL transaction rollback", async {
                    transaction.rollback().await.map_err(DbError::from)
                })
                .await;
                Err(error)
            }
        }
    }

    /// Cancels the statement this session is running.
    pub async fn cancel_query(&self, token: tokio_postgres::CancelToken) -> Result<(), DbError> {
        let connection = &self.connection;
        let limits = self.limits;
        query_timeout(limits, "PostgreSQL query cancellation", async {
            match (connection.sslmode, &connection.socks5_proxy) {
                (PostgresSslMode::Disable, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                        limits.connection_timeout,
                    )
                    .await?;
                    token
                        .cancel_query_raw(stream, NoTls)
                        .await
                        .map_err(DbError::database_connection)
                }
                (PostgresSslMode::Disable, None) => token
                    .cancel_query(NoTls)
                    .await
                    .map_err(DbError::database_connection),
                (mode, Some(proxy)) => {
                    let stream = connect_socks5(
                        proxy,
                        self.socks5_password.as_deref().map(String::as_str),
                        &connection.host,
                        connection.port,
                        limits.connection_timeout,
                    )
                    .await?;
                    let mut tls =
                        postgres_tls_connector(mode, connection.trust_ca_file.as_deref())?;
                    let tls = <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(
                        &mut tls,
                        &connection.host,
                    )
                    .map_err(|error| {
                        DbError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
                    })?;
                    token
                        .cancel_query_raw(stream, tls)
                        .await
                        .map_err(DbError::database_connection)
                }
                (mode, None) => token
                    .cancel_query(postgres_tls_connector(
                        mode,
                        connection.trust_ca_file.as_deref(),
                    )?)
                    .await
                    .map_err(DbError::database_connection),
            }
        })
        .await
    }

    /// Closes the session, giving the driver time to wind down.
    pub async fn close(self) -> Result<(), DbError> {
        let close_timeout = self.limits.postgres_close_timeout;
        drop(self.client);
        await_postgres_driver(self.driver, close_timeout).await
    }
}

/// Decodes the first `column_count` values of one PostgreSQL row.
fn decode_postgres_row(
    row: Result<tokio_postgres::Row, tokio_postgres::Error>,
    column_count: usize,
) -> Result<Vec<Cell>, DbError> {
    let row = row?;
    (0..column_count)
        .map(|index| {
            row.try_get::<_, PostgresCell>(index)
                .map(|cell| cell.0)
                .map_err(DbError::from)
        })
        .collect()
}

pub(crate) async fn await_postgres_driver(
    mut driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    close_timeout: Duration,
) -> Result<(), DbError> {
    match timeout(close_timeout, &mut driver).await {
        Ok(result) => result
            .map_err(|error| {
                DbError::Database(format!("PostgreSQL connection task failed: {error}"))
            })?
            .map_err(DbError::database_connection),
        Err(_) => {
            driver.abort();
            let _ = driver.await;
            Err(DbError::Database(format!(
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
    DbError,
> {
    match timeout(connect_timeout, configuration.connect_raw(stream, NoTls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(DbError::database_connection(error)),
        Err(_) => Err(DbError::Database(format!(
            "PostgreSQL startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}

pub(crate) async fn connect_postgres_raw_tls(
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
    DbError,
> {
    let tls =
        <MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(&mut tls, hostname)
            .map_err(|error| {
                DbError::Data(format!("invalid PostgreSQL TLS server name: {error}"))
            })?;
    match timeout(connect_timeout, configuration.connect_raw(stream, tls)).await {
        Ok(Ok((client, connection_driver))) => Ok((client, tokio::spawn(connection_driver))),
        Ok(Err(error)) => Err(DbError::database_connection(error)),
        Err(_) => Err(DbError::Database(format!(
            "PostgreSQL TLS startup through SOCKS5 timed out after {connect_timeout:?}"
        ))),
    }
}
