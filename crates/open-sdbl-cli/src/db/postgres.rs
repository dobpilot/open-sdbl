use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use open_sdbl::metadata::{
    LiveTable, MetadataSnapshot, PostgresMetadataQueries, parse_db_names, parse_schema_storage,
};
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
use tokio_postgres::types::{FromSql, ToSql, Type};
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};
use tokio_postgres_rustls::MakeRustlsConnect;
use zeroize::Zeroizing;

use crate::args::{PostgresConnection, PostgresSslMode};
use crate::auth::pgpass::{Credentials, postgres_password};
use crate::cells::{Cell, DAYS_FROM_UNIX_EPOCH_TO_2000, DateTimeParts, decode_postgres_numeric};
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

fn postgres_tls_connector(
    mode: PostgresSslMode,
    trust_ca_file: Option<&str>,
) -> Result<MakeRustlsConnect, CliError> {
    let signature_roots = Arc::new(RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    });
    let signature_verifier = WebPkiServerVerifier::builder(Arc::clone(&signature_roots))
        .build()
        .map_err(|error| CliError::Data(format!("cannot configure PostgreSQL TLS: {error}")))?;

    let certificate_roots = match mode {
        PostgresSslMode::Require => None,
        PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull => {
            let mut roots = RootCertStore::empty();
            let (accepted, details) = if let Some(path) = trust_ca_file {
                let bytes = std::fs::read(path).map_err(|error| {
                    CliError::Io(
                        format!("cannot read PostgreSQL CA certificate file {path:?}"),
                        error,
                    )
                })?;
                let pem_certificates = CertificateDer::pem_slice_iter(&bytes)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        CliError::Data(format!(
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
                return Err(CliError::Data(format!(
                    "cannot load any CA certificates for PostgreSQL TLS{}",
                    details.unwrap_or_default()
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
                    postgres_tls_connector(mode, connection.trust_ca_file.as_deref())?,
                    &connection.host,
                    CONNECTION_TIMEOUT,
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
                            .map(|index| {
                                row.try_get::<_, PostgresCell>(index)
                                    .map(|cell| cell.0)
                                    .map_err(CliError::from)
                            })
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
                    let mut tls =
                        postgres_tls_connector(mode, connection.trust_ca_file.as_deref())?;
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
                    .cancel_query(postgres_tls_connector(
                        mode,
                        connection.trust_ca_file.as_deref(),
                    )?)
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
        decode_config_stream(
            resources,
            CONFIG_DECODE_BATCH_SIZE,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
        )
        .await
    }

    async fn read_extension_resources(&mut self) -> Result<Vec<ConfigResource>, CliError> {
        let parameters = std::iter::empty::<&(dyn ToSql + Sync)>();
        let rows = query_timeout("PostgreSQL ConfigCAS query", async {
            self.transaction()?
                .query_raw(PostgresMetadataQueries::EXTENSION_RESOURCES, parameters)
                .await
                .map_err(CliError::from)
        })
        .await?;
        tokio::pin!(rows);
        let mut resources = Vec::new();
        while let Some(row) = rows.next().await {
            let row = row?;
            resources.push(ConfigResource {
                file_name: row.try_get(0)?,
                compressed: row.try_get(1)?,
            });
        }
        Ok(resources)
    }

    async fn read_extension_restructures(&mut self) -> Result<Vec<Vec<u8>>, CliError> {
        let rows = postgres_rows(
            self.transaction()?,
            "PostgreSQL extension restructure query",
            PostgresMetadataQueries::EXTENSION_RESTRUCTURE,
        )
        .await?;
        rows.iter()
            .map(|row| row.try_get::<_, Vec<u8>>(0).map_err(CliError::from))
            .collect()
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

/// Decodes one binary-protocol PostgreSQL value into a typed [`Cell`].
///
/// Only the types the compiler can emit are decoded; anything else is a data
/// error naming the type, so undocumented wire formats never print as garbage.
struct PostgresCell(Cell);

impl<'a> FromSql<'a> for PostgresCell {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        decode_postgres_cell(ty, raw).map(Self).map_err(Into::into)
    }

    fn from_sql_null(_: &Type) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(Cell::Null))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

fn decode_postgres_cell(ty: &Type, raw: &[u8]) -> Result<Cell, String> {
    fn array<const N: usize>(raw: &[u8], ty: &Type) -> Result<[u8; N], String> {
        <[u8; N]>::try_from(raw).map_err(|_| {
            format!(
                "PostgreSQL {} value has {} bytes, expected {N}",
                ty.name(),
                raw.len()
            )
        })
    }

    Ok(match *ty {
        Type::BYTEA => Cell::Bytes(raw.to_vec()),
        Type::UUID => Cell::Uuid(array(raw, ty)?),
        Type::BOOL => Cell::Bool(array::<1>(raw, ty)?[0] != 0),
        Type::INT2 => Cell::Number(i16::from_be_bytes(array(raw, ty)?).to_string()),
        Type::INT4 => Cell::Number(i32::from_be_bytes(array(raw, ty)?).to_string()),
        Type::INT8 => Cell::Number(i64::from_be_bytes(array(raw, ty)?).to_string()),
        Type::FLOAT4 => Cell::Number(f32::from_be_bytes(array(raw, ty)?).to_string()),
        Type::FLOAT8 => Cell::Number(f64::from_be_bytes(array(raw, ty)?).to_string()),
        Type::NUMERIC => Cell::Number(decode_postgres_numeric(raw)?),
        Type::TIMESTAMP => {
            let microseconds = i64::from_be_bytes(array(raw, ty)?);
            let seconds = microseconds.div_euclid(1_000_000);
            Cell::DateTime(DateTimeParts::from_unix_days(
                seconds.div_euclid(86_400) + DAYS_FROM_UNIX_EPOCH_TO_2000,
                u32::try_from(seconds.rem_euclid(86_400)).unwrap_or(0),
            ))
        }
        Type::DATE => {
            let days = i64::from(i32::from_be_bytes(array(raw, ty)?));
            Cell::DateTime(DateTimeParts::from_unix_days(
                days + DAYS_FROM_UNIX_EPOCH_TO_2000,
                0,
            ))
        }
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN => Cell::Text(
            std::str::from_utf8(raw)
                .map_err(|error| format!("PostgreSQL {} value is not UTF-8: {error}", ty.name()))?
                .to_owned(),
        ),
        _ => {
            return Err(format!(
                "unsupported PostgreSQL column type {}; the compiler should have cast it",
                ty.name()
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_postgres::types::Type;

    use super::{await_postgres_driver, decode_postgres_cell};
    use crate::cells::{Cell, DateTimeParts};

    #[test]
    fn decodes_binary_protocol_values_into_typed_cells() {
        assert_eq!(
            decode_postgres_cell(&Type::BYTEA, &[0, 0x7d, 0xd6]).unwrap(),
            Cell::Bytes(vec![0, 0x7d, 0xd6])
        );
        assert_eq!(
            decode_postgres_cell(&Type::UUID, &[9; 16]).unwrap(),
            Cell::Uuid([9; 16])
        );
        assert_eq!(
            decode_postgres_cell(&Type::BOOL, &[1]).unwrap(),
            Cell::Bool(true)
        );
        assert_eq!(
            decode_postgres_cell(&Type::INT8, &(-42_i64).to_be_bytes()).unwrap(),
            Cell::Number("-42".to_owned())
        );
        assert_eq!(
            decode_postgres_cell(&Type::FLOAT8, &1.5_f64.to_be_bytes()).unwrap(),
            Cell::Number("1.5".to_owned())
        );
        // 2024-02-29 12:34:56 is 762_525_296 seconds after 2000-01-01.
        assert_eq!(
            decode_postgres_cell(&Type::TIMESTAMP, &(762_525_296_000_000_i64).to_be_bytes())
                .unwrap(),
            Cell::DateTime(DateTimeParts {
                year: 2024,
                month: 2,
                day: 29,
                hour: 12,
                minute: 34,
                second: 56,
            })
        );
        assert_eq!(
            decode_postgres_cell(&Type::DATE, &(-1_i32).to_be_bytes()).unwrap(),
            Cell::DateTime(DateTimeParts {
                year: 1999,
                month: 12,
                day: 31,
                hour: 0,
                minute: 0,
                second: 0,
            })
        );
        assert_eq!(
            decode_postgres_cell(&Type::TEXT, "Код".as_bytes()).unwrap(),
            Cell::Text("Код".to_owned())
        );
        assert!(
            decode_postgres_cell(&Type::JSON, b"{}")
                .unwrap_err()
                .contains("json")
        );
        assert!(decode_postgres_cell(&Type::INT4, &[1, 2]).is_err());
    }

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
