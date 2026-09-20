//! One session over a database provider.
//!
//! An application speaks to PostgreSQL and to SQL Server through the same
//! three steps — connect, read metadata, close — so the choice of provider
//! is made once, here, and everything above works against this facade.

use std::future::Future;
use std::time::Duration;

use open_sdbl::metadata::{MetadataSnapshot, ResolutionReport, StorageLayout};
use open_sdbl::query::MsSqlBackend;
use tokio::time::timeout;

use crate::cells::{Cell, QueryRows, RowFlow};
use crate::connection::DatabaseConnection;
use crate::credentials::Credentials;
use crate::db::mssql::MsSqlSession;
use crate::db::postgres::PostgresSession;
use crate::error::DbError;
use crate::limits::Limits;
use crate::progress::MetadataProgress;

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;

/// Runs one database call under a limit, naming it in the timeout error.
pub async fn bounded_database_call<T>(
    label: &str,
    duration: Duration,
    future: impl Future<Output = Result<T, DbError>>,
) -> Result<T, DbError> {
    timeout(duration, future)
        .await
        .map_err(|_| DbError::DatabaseTimeout {
            operation: label.to_owned(),
            duration,
        })?
}

/// Runs one database call under the query limit of `limits`.
pub async fn query_timeout<T>(
    limits: Limits,
    label: &str,
    future: impl Future<Output = Result<T, DbError>>,
) -> Result<T, DbError> {
    bounded_database_call(label, limits.query_timeout, future).await
}

/// Which provider a session speaks to, and how its SQL is generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseDialect {
    /// PostgreSQL.
    Postgres,
    /// Microsoft SQL Server, with the backend the server was detected as.
    MsSql {
        /// The backend generated T-SQL is bound to.
        backend: MsSqlBackend,
    },
}

/// One open session over a database provider.
pub enum DatabaseSession {
    /// A PostgreSQL session.
    Postgres(Box<PostgresSession>),
    /// A Microsoft SQL Server session.
    MsSql(Box<MsSqlSession>),
}

/// A handle that cancels the statement a session is running.
pub enum QueryCancellation {
    /// The PostgreSQL cancellation key of the running statement.
    Postgres(tokio_postgres::CancelToken),
    /// SQL Server is cancelled by dropping and reopening the session.
    MsSql,
}

impl DatabaseSession {
    /// Opens a session against the described base under `limits`.
    pub async fn connect(
        connection: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        match connection {
            DatabaseConnection::Postgres(connection) => {
                PostgresSession::connect(connection, credentials, limits)
                    .await
                    .map(Box::new)
                    .map(Self::Postgres)
            }
            DatabaseConnection::MsSql(connection) => {
                MsSqlSession::connect(connection, credentials, limits)
                    .await
                    .map(Box::new)
                    .map(Self::MsSql)
            }
        }
    }

    /// The provider this session speaks to.
    pub const fn dialect(&self) -> DatabaseDialect {
        match self {
            Self::Postgres(_) => DatabaseDialect::Postgres,
            Self::MsSql(session) => DatabaseDialect::MsSql {
                backend: session.backend(),
            },
        }
    }

    /// Provider-specific startup line, when the provider has one.
    pub fn server_description(&self) -> Option<String> {
        match self {
            Self::Postgres(_) => None,
            Self::MsSql(session) => Some(session.server_description()),
        }
    }

    /// How a failure of this provider is named in a message.
    pub const fn execution_label(&self) -> &'static str {
        match self {
            Self::Postgres(_) => "PostgreSQL execution",
            Self::MsSql(_) => "MSSQL execution",
        }
    }

    /// Whether the session can no longer be used.
    pub fn is_dead(&self) -> bool {
        match self {
            Self::Postgres(session) => session.is_closed(),
            Self::MsSql(session) => session.is_dead(),
        }
    }

    /// A handle that cancels whatever this session is running.
    pub fn cancellation(&self) -> QueryCancellation {
        match self {
            Self::Postgres(session) => QueryCancellation::Postgres(session.cancellation()),
            Self::MsSql(_) => QueryCancellation::MsSql,
        }
    }

    /// Cancels the running statement, reconnecting where the provider
    /// offers no other way.
    pub async fn cancel_query(&mut self, cancellation: QueryCancellation) -> Result<(), DbError> {
        match (self, cancellation) {
            (Self::Postgres(session), QueryCancellation::Postgres(token)) => {
                session.cancel_query(token).await
            }
            (Self::MsSql(session), QueryCancellation::MsSql) => {
                session.cancel_and_reconnect().await
            }
            _ => Err(DbError::Database(
                "database session changed while cancelling a query".to_owned(),
            )),
        }
    }

    /// Reads and resolves the metadata of the base, answering the
    /// snapshot and the report of what the resolver had to recover from.
    pub async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        match self {
            Self::Postgres(session) => session.metadata(progress).await,
            Self::MsSql(session) => session.metadata(progress).await,
        }
    }

    /// The storage layout of the base, known after a metadata read.
    pub fn layout(&self) -> Option<StorageLayout> {
        match self {
            Self::Postgres(session) => session.layout(),
            Self::MsSql(session) => session.layout(),
        }
    }

    /// Runs one read-only statement and decodes `column_count` columns.
    ///
    /// The whole result is held in memory. A caller that cannot promise
    /// the result is small reads it with [`DatabaseSession::query_each`]
    /// instead.
    pub async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        match self {
            Self::Postgres(session) => session.query(sql, column_count).await,
            Self::MsSql(session) => session.query(sql, column_count).await,
        }
    }

    /// Reads one read-only statement row by row.
    ///
    /// Each row is decoded on its own and handed to `on_row` before the
    /// next is read from the server, so a result larger than memory is
    /// readable. Answering [`RowFlow::Stop`] ends the read at once: the
    /// rows that follow are never fetched, decoded, or allocated.
    ///
    /// Stopping early costs a SQL Server session its connection — that is
    /// the only way to end a running statement there, and the session
    /// reports [`DatabaseSession::is_dead`] afterwards. A PostgreSQL
    /// session stays usable.
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
        match self {
            Self::Postgres(session) => session.query_each(sql, column_count, on_row).await,
            Self::MsSql(session) => session.query_each(sql, column_count, on_row).await,
        }
    }

    /// Closes the session.
    pub async fn close(self) -> Result<(), DbError> {
        match self {
            Self::Postgres(session) => session.close().await,
            Self::MsSql(session) => session.close().await,
        }
    }
}
