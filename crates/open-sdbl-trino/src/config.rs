use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use tokio_postgres::Config as PostgresConfig;

use crate::{ErrorCode, ServiceError};

/// Process configuration. Secret values deliberately have no `Debug` impl.
#[derive(Clone)]
pub struct ServiceConfig {
    pub listen: SocketAddr,
    pub database: PostgresConfig,
    pub tls: DatabaseTls,
    pub metadata_cache_ttl: Duration,
    pub pool_size: usize,
    pub pool_create_timeout: Duration,
    pub pool_wait_timeout: Duration,
    pub statement_timeout: Duration,
    pub query_timeout: Duration,
    pub maximum_result_rows: Option<u64>,
    pub config_decode_batch_size: usize,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseTls {
    Disable,
    Native,
}

impl ServiceConfig {
    /// Reads documented `OPEN_SDBL_*` and PostgreSQL environment variables.
    pub fn from_env() -> Result<Self, ServiceError> {
        let listen = value("OPEN_SDBL_LISTEN")
            .unwrap_or_else(|| "0.0.0.0:8088".to_owned())
            .parse()
            .map_err(|error| configuration(format!("invalid OPEN_SDBL_LISTEN: {error}")))?;
        let (mut database, tls) = database_config()?;
        let postgres_connect_timeout =
            Duration::from_millis(unsigned("OPEN_SDBL_POSTGRES_CONNECT_TIMEOUT_MS", 10_000)?);
        database.connect_timeout(postgres_connect_timeout);
        Ok(Self {
            listen,
            database,
            tls,
            metadata_cache_ttl: Duration::from_secs(unsigned("OPEN_SDBL_METADATA_CACHE_TTL", 300)?),
            pool_size: usize::try_from(unsigned("OPEN_SDBL_POSTGRES_POOL_SIZE", 8)?)
                .map_err(|_| configuration("OPEN_SDBL_POSTGRES_POOL_SIZE is too large"))?
                .max(1),
            pool_create_timeout: Duration::from_millis(unsigned(
                "OPEN_SDBL_POSTGRES_POOL_CREATE_TIMEOUT_MS",
                10_000,
            )?),
            pool_wait_timeout: Duration::from_millis(unsigned(
                "OPEN_SDBL_POSTGRES_POOL_WAIT_TIMEOUT_MS",
                10_000,
            )?),
            statement_timeout: Duration::from_millis(unsigned(
                "OPEN_SDBL_STATEMENT_TIMEOUT_MS",
                60_000,
            )?),
            query_timeout: Duration::from_millis(unsigned("OPEN_SDBL_QUERY_TIMEOUT_MS", 65_000)?),
            maximum_result_rows: optional_unsigned("OPEN_SDBL_MAXIMUM_RESULT_ROWS")?,
            config_decode_batch_size: usize::try_from(unsigned(
                "OPEN_SDBL_CONFIG_DECODE_BATCH_SIZE",
                256,
            )?)
            .map_err(|_| configuration("OPEN_SDBL_CONFIG_DECODE_BATCH_SIZE is too large"))?
            .max(1),
            refresh_token: value("OPEN_SDBL_REFRESH_TOKEN"),
        })
    }
}

fn database_config() -> Result<(PostgresConfig, DatabaseTls), ServiceError> {
    if let Some(url) = value("OPEN_SDBL_DATABASE_URL") {
        let config = PostgresConfig::from_str(&url)
            .map_err(|error| configuration(format!("invalid OPEN_SDBL_DATABASE_URL: {error}")))?;
        let tls = if config.get_ssl_mode() != tokio_postgres::config::SslMode::Disable {
            DatabaseTls::Native
        } else {
            DatabaseTls::Disable
        };
        return Ok((config, tls));
    }
    let mut config = PostgresConfig::new();
    config.host(value("OPEN_SDBL_POSTGRES_HOST").unwrap_or_else(|| "localhost".to_owned()));
    config.port(
        u16::try_from(unsigned("OPEN_SDBL_POSTGRES_PORT", 5432)?)
            .map_err(|_| configuration("OPEN_SDBL_POSTGRES_PORT is outside 1..=65535"))?,
    );
    config.dbname(
        &value("OPEN_SDBL_POSTGRES_DATABASE")
            .ok_or_else(|| configuration("OPEN_SDBL_POSTGRES_DATABASE is required"))?,
    );
    config.user(
        &value("OPEN_SDBL_POSTGRES_USERNAME")
            .or_else(|| value("PGUSER"))
            .ok_or_else(|| configuration("OPEN_SDBL_POSTGRES_USERNAME is required"))?,
    );
    if let Some(password) = value("OPEN_SDBL_POSTGRES_PASSWORD").or_else(|| value("PGPASSWORD")) {
        config.password(password);
    }
    let tls_mode = value("OPEN_SDBL_POSTGRES_TLS_MODE").unwrap_or_else(|| "disable".to_owned());
    let tls = match tls_mode.to_ascii_lowercase().as_str() {
        "disable" => DatabaseTls::Disable,
        "prefer" => {
            config.ssl_mode(tokio_postgres::config::SslMode::Prefer);
            DatabaseTls::Native
        }
        "require" => {
            config.ssl_mode(tokio_postgres::config::SslMode::Require);
            DatabaseTls::Native
        }
        _ => {
            return Err(configuration(
                "OPEN_SDBL_POSTGRES_TLS_MODE must be disable, prefer, or require",
            ));
        }
    };
    Ok((config, tls))
}

fn value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn unsigned(name: &str, default: u64) -> Result<u64, ServiceError> {
    value(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|error| configuration(format!("invalid {name}: {error}")))
    })
}

fn optional_unsigned(name: &str) -> Result<Option<u64>, ServiceError> {
    value(name)
        .map(|value| {
            value
                .parse()
                .map_err(|error| configuration(format!("invalid {name}: {error}")))
        })
        .transpose()
}

fn configuration(message: impl Into<String>) -> ServiceError {
    ServiceError::new(ErrorCode::Protocol, message)
}

#[cfg(test)]
mod tests {
    use super::configuration;

    #[test]
    fn configuration_error_does_not_require_secret_rendering() {
        let error = configuration("invalid database configuration");
        assert_eq!(error.to_string(), "invalid database configuration");
    }
}
