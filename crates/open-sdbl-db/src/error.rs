//! The error every database operation of this crate answers with.

use std::fmt;
use std::io;
use std::time::Duration;

use open_sdbl::metadata::MetadataError;

/// What went wrong while talking to a database or decoding what it sent.
///
/// The variants separate what the operator can act on — a malformed
/// resource, a refused connection, a call that ran too long — so an
/// application can decide its own exit status from the kind alone.
#[derive(Debug)]
#[non_exhaustive]
pub enum DbError {
    /// An operating-system failure, with what was being attempted.
    Io(String, io::Error),
    /// Metadata the library could not decode.
    Metadata(MetadataError),
    /// Data a database returned that does not match what was expected.
    Data(String),
    /// A connection, a query, or a transaction the database refused.
    Database(String),
    /// A call that exceeded the limit it was given.
    DatabaseTimeout {
        /// What was being attempted.
        operation: String,
        /// The limit that was exceeded.
        duration: Duration,
    },
    /// A failure reported by the SQL Server driver.
    MsSql {
        /// What was being attempted.
        operation: &'static str,
        /// The driver error.
        source: tiberius::error::Error,
    },
}

impl DbError {
    /// A PostgreSQL connection that could not be opened.
    pub fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL connection failed: {error}"))
    }

    /// A SQL Server connection that could not be opened.
    pub const fn mssql_connection(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "connection",
            source: error,
        }
    }

    /// A SQL Server statement that failed.
    pub const fn mssql_query(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "query",
            source: error,
        }
    }

    /// A SOCKS5 proxy that refused or dropped the connection.
    pub fn socks5_connection(error: impl fmt::Display) -> Self {
        Self::Database(format!("SOCKS5 proxy connection failed: {error}"))
    }

    /// Whether a call was abandoned because it exceeded its limit.
    pub const fn is_database_timeout(&self) -> bool {
        matches!(self, Self::DatabaseTimeout { .. })
    }

    /// Whether the SQL Server connection itself failed, rather than the
    /// statement sent over it.
    pub const fn is_mssql_connection_failure(&self) -> bool {
        matches!(
            self,
            Self::MsSql {
                source: tiberius::error::Error::Io { .. }
                    | tiberius::error::Error::Protocol(_)
                    | tiberius::error::Error::Tls(_),
                ..
            }
        )
    }

    /// Whether the SQL Server session must be dropped rather than reused:
    /// a timed-out call may have left the TDS stream between messages.
    pub const fn requires_mssql_disconnect(&self) -> bool {
        self.is_database_timeout() || self.is_mssql_connection_failure()
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Metadata(error) => error.fmt(formatter),
            Self::MsSql { operation, source } => {
                write!(formatter, "MSSQL {operation} failed: {source}")
            }
            Self::DatabaseTimeout {
                operation,
                duration,
            } => write!(
                formatter,
                "{operation} timed out after {:.3} seconds",
                duration.as_secs_f64()
            ),
            Self::Data(message) | Self::Database(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(_, error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::MsSql { source, .. } => Some(source),
            Self::Data(_) | Self::Database(_) | Self::DatabaseTimeout { .. } => None,
        }
    }
}

impl From<MetadataError> for DbError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<tokio_postgres::Error> for DbError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL query failed: {error}"))
    }
}
