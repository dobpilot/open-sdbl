//! One session over a database provider.
//!
//! The CLI speaks to PostgreSQL and to SQL Server through the same three
//! steps — connect, read metadata, close — so the choice of provider is
//! made once, here, and every command above works against this facade.

use std::future::Future;
use std::time::Duration;

use open_sdbl::metadata::{MetadataSnapshot, StorageLayout};
use open_sdbl::query::MsSqlBackend;
use tokio::time::timeout;

use crate::args::DatabaseConnection;
use crate::auth::pgpass::Credentials;
use crate::cells::QueryRows;
use crate::db::mssql::MsSqlSession;
use crate::db::postgres::PostgresSession;
use crate::error::CliError;
use crate::limits::QUERY_TIMEOUT;

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;

pub(crate) async fn bounded_database_call<T>(
    label: &str,
    duration: Duration,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    timeout(duration, future)
        .await
        .map_err(|_| CliError::DatabaseTimeout {
            operation: label.to_owned(),
            duration,
        })?
}

pub(crate) async fn query_timeout<T>(
    label: &str,
    future: impl Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
    bounded_database_call(label, QUERY_TIMEOUT, future).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatabaseDialect {
    Postgres,
    MsSql { backend: MsSqlBackend },
}

pub(crate) enum DatabaseSession {
    Postgres(Box<PostgresSession>),
    MsSql(Box<MsSqlSession>),
}

pub(crate) enum QueryCancellation {
    Postgres(tokio_postgres::CancelToken),
    MsSql,
}

impl DatabaseSession {
    pub(crate) async fn connect(
        connection: &DatabaseConnection,
        credentials: &Credentials,
    ) -> Result<Self, CliError> {
        match connection {
            DatabaseConnection::Postgres(connection) => {
                PostgresSession::connect(connection, credentials)
                    .await
                    .map(Box::new)
                    .map(Self::Postgres)
            }
            DatabaseConnection::MsSql(connection) => MsSqlSession::connect(connection, credentials)
                .await
                .map(Box::new)
                .map(Self::MsSql),
        }
    }

    pub(crate) const fn dialect(&self) -> DatabaseDialect {
        match self {
            Self::Postgres(_) => DatabaseDialect::Postgres,
            Self::MsSql(session) => DatabaseDialect::MsSql {
                backend: session.backend(),
            },
        }
    }

    /// Provider-specific startup line, when the provider has one.
    pub(crate) fn server_description(&self) -> Option<String> {
        match self {
            Self::Postgres(_) => None,
            Self::MsSql(session) => Some(session.server_description()),
        }
    }

    pub(crate) const fn execution_label(&self) -> &'static str {
        match self {
            Self::Postgres(_) => "PostgreSQL execution",
            Self::MsSql(_) => "MSSQL execution",
        }
    }

    pub(crate) fn is_dead(&self) -> bool {
        match self {
            Self::Postgres(session) => session.is_closed(),
            Self::MsSql(session) => session.is_dead(),
        }
    }

    pub(crate) fn cancellation(&self) -> QueryCancellation {
        match self {
            Self::Postgres(session) => QueryCancellation::Postgres(session.cancellation()),
            Self::MsSql(_) => QueryCancellation::MsSql,
        }
    }

    pub(crate) async fn cancel_query(
        &mut self,
        cancellation: QueryCancellation,
    ) -> Result<(), CliError> {
        match (self, cancellation) {
            (Self::Postgres(session), QueryCancellation::Postgres(token)) => {
                session.cancel_query(token).await
            }
            (Self::MsSql(session), QueryCancellation::MsSql) => {
                session.cancel_and_reconnect().await
            }
            _ => Err(CliError::Database(
                "database session changed while cancelling a query".to_owned(),
            )),
        }
    }

    pub(crate) async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        match self {
            Self::Postgres(session) => session.metadata().await,
            Self::MsSql(session) => session.metadata().await,
        }
    }

    /// The storage layout of the base, known after a metadata read.
    pub(crate) fn layout(&self) -> Option<StorageLayout> {
        match self {
            Self::Postgres(session) => session.layout(),
            Self::MsSql(session) => session.layout(),
        }
    }

    pub(crate) async fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> Result<QueryRows, CliError> {
        match self {
            Self::Postgres(session) => session.query(sql, column_count).await,
            Self::MsSql(session) => session.query(sql, column_count).await,
        }
    }

    pub(crate) async fn close(self) -> Result<(), CliError> {
        match self {
            Self::Postgres(session) => session.close().await,
            Self::MsSql(session) => session.close().await,
        }
    }
}
