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
    /// Для безопасного разрешения выбранных объектов не приобретены описатели.
    ConfigurationDescriptorsMissing {
        /// GUID недостающих описателей в детерминированном порядке, без повторов.
        objects: Vec<open_sdbl::metadata::Guid>,
    },
    /// A restriction needs a session parameter whose value is absent.
    MissingSessionParameter {
        /// The parameter name as written in the restriction.
        name: String,
        /// The object and user whose restriction needed it.
        context: String,
    },
    /// A connection, a query, or a transaction the database refused.
    Database(String),
    /// A PostgreSQL driver error with the operation that was attempted.
    Postgres {
        /// The operation, without SQL or connection parameters.
        operation: String,
        /// The original driver error.
        source: tokio_postgres::Error,
    },
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
    /// An operation called on a blocking session after it was closed.
    SessionClosed,
    /// A blocking operation called on a thread that is already driving an
    /// asynchronous runtime, where blocking on another would panic.
    AsyncContext,
}

impl DbError {
    /// A PostgreSQL connection that could not be opened.
    pub fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::postgres("PostgreSQL connection", error)
    }

    pub(crate) fn postgres(operation: &str, source: tokio_postgres::Error) -> Self {
        Self::Postgres {
            operation: operation.to_owned(),
            source,
        }
    }

    pub(crate) fn with_postgres_operation(self, label: &str) -> Self {
        match self {
            Self::Postgres { operation, source } if operation == "PostgreSQL query" => {
                Self::postgres(label, source)
            }
            other => other,
        }
    }

    /// Safe PostgreSQL diagnostic fields for operator and optional wire output.
    /// Server text can contain SQL, values, and credentials. Only the SQLSTATE
    /// and descriptions derived from known operations and error kinds are used.
    pub fn postgres_diagnostic(&self) -> Option<(&str, String, Option<&'static str>)> {
        let Self::Postgres { operation, source } = self else {
            return None;
        };
        let code = source
            .code()
            .map(tokio_postgres::error::SqlState::code)
            .filter(|code| {
                code.len() == 5
                    && code
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            });
        let (description, next) = match code {
            Some("42P01") if operation == "PostgreSQL DBNames query" => (
                "Relation \"params\" does not exist",
                Some("Check that --database identifies a 1C information base."),
            ),
            Some("42P01") => ("A required relation does not exist", None),
            Some("42501") => ("Permission denied", None),
            Some("22012") => ("Division by zero", None),
            Some("28P01") => ("Password authentication failed", None),
            Some("3D000") => ("The database does not exist", None),
            Some("57014") => ("The server cancelled the query", None),
            Some(_) => ("The server rejected the operation", None),
            None => {
                let mut error = std::error::Error::source(source);
                let mut io_kind = None;
                while let Some(cause) = error {
                    if let Some(io) = cause.downcast_ref::<io::Error>() {
                        io_kind = Some(io.kind());
                        // Error::source can skip the error wrapped by io::Error.
                        if let Some(inner) = io.get_ref() {
                            error = Some(inner);
                            continue;
                        }
                    }
                    if let Some(tls) = cause.downcast_ref::<rustls::Error>() {
                        use rustls::{CertificateError, Error};

                        let (description, next) = match tls {
                            Error::InvalidCertificate(CertificateError::UnknownIssuer) => (
                                "The server's TLS certificate issuer is not trusted",
                                Some(
                                    "If the server uses a private CA, supply it with --trust-ca-file.",
                                ),
                            ),
                            Error::InvalidCertificate(
                                CertificateError::NotValidForName
                                | CertificateError::NotValidForNameContext { .. },
                            ) => (
                                "The server's TLS certificate does not match the connection hostname",
                                Some("Use the hostname in the server's certificate with --host."),
                            ),
                            _ => ("TLS negotiation or certificate validation failed", None),
                        };
                        return Some((operation, description.into(), next));
                    }
                    error = cause.source();
                }
                let description =
                    io_kind.map_or_else(|| source.to_string(), |kind| kind.to_string());
                return Some((operation, description, None));
            }
        };
        Some((
            operation,
            format!(
                "{description} (SQLSTATE {})",
                code.expect("matched SQLSTATE")
            ),
            next,
        ))
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

    /// Whether the call was made on a session that was already closed.
    pub const fn is_session_closed(&self) -> bool {
        matches!(self, Self::SessionClosed)
    }

    /// The name of a session parameter a restriction could not read.
    pub fn missing_session_parameter(&self) -> Option<&str> {
        match self {
            Self::MissingSessionParameter { name, .. } => Some(name),
            _ => None,
        }
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
                if let Some(code) = source.code() {
                    write!(
                        formatter,
                        "MSSQL {operation} failed (SQL Server error {code})"
                    )
                } else {
                    let cause = match source {
                        tiberius::error::Error::Tls(_) => {
                            "TLS negotiation or certificate validation failed"
                        }
                        tiberius::error::Error::Io { message, .. }
                            if message == crate::db::mssql::transport::ENCRYPTION_REFUSED =>
                        {
                            crate::db::mssql::transport::ENCRYPTION_REFUSED
                        }
                        tiberius::error::Error::Io { message, .. }
                            if message == crate::db::mssql::transport::INVALID_PRELOGIN =>
                        {
                            crate::db::mssql::transport::INVALID_PRELOGIN
                        }
                        tiberius::error::Error::Io { .. } => "transport I/O failed",
                        tiberius::error::Error::Protocol(_) => "invalid server protocol response",
                        _ => "driver error",
                    };
                    write!(formatter, "MSSQL {operation} failed: {cause}")
                }
            }
            Self::Postgres { .. } => {
                let (operation, cause, _) = self.postgres_diagnostic().expect("PostgreSQL error");
                write!(formatter, "{operation} failed: {cause}")
            }
            Self::DatabaseTimeout {
                operation,
                duration,
            } => write!(
                formatter,
                "{operation} timed out after {:.3} seconds",
                duration.as_secs_f64()
            ),
            Self::ConfigurationDescriptorsMissing { objects } => {
                write!(formatter, "configuration descriptors are missing: ")?;
                for (index, guid) in objects.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(", ")?;
                    }
                    guid.fmt(formatter)?;
                }
                Ok(())
            }
            Self::Data(message) | Self::Database(message) => formatter.write_str(message),
            Self::MissingSessionParameter { name, context } => {
                write!(
                    formatter,
                    "{context}: session parameter \"{name}\" has no value"
                )
            }
            Self::SessionClosed => formatter.write_str("the database session is closed"),
            Self::AsyncContext => formatter
                .write_str("a blocking database call was made from inside an asynchronous runtime"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(_, error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::MsSql { source, .. } => Some(source),
            Self::Postgres { source, .. } => Some(source),
            Self::ConfigurationDescriptorsMissing { .. }
            | Self::Data(_)
            | Self::MissingSessionParameter { .. }
            | Self::Database(_)
            | Self::DatabaseTimeout { .. }
            | Self::SessionClosed
            | Self::AsyncContext => None,
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
        Self::postgres("PostgreSQL query", error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_postgres::tls::TlsConnect;

    struct RejectedCertificate(rustls::CertificateError);

    impl TlsConnect<tokio::io::DuplexStream> for RejectedCertificate {
        type Stream = <tokio_postgres::NoTls as TlsConnect<tokio::io::DuplexStream>>::Stream;
        type Error = io::Error;
        type Future = std::future::Ready<Result<Self::Stream, Self::Error>>;

        fn connect(self, _: tokio::io::DuplexStream) -> Self::Future {
            std::future::ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                rustls::Error::InvalidCertificate(self.0),
            )))
        }
    }

    #[tokio::test]
    async fn safe_diagnostic_recognizes_wrapped_tls_certificate_errors() {
        for (certificate_error, expected, flag) in [
            (
                rustls::CertificateError::UnknownIssuer,
                "issuer is not trusted",
                "--trust-ca-file",
            ),
            (
                rustls::CertificateError::NotValidForName,
                "does not match",
                "--host",
            ),
        ] {
            let (input, mut server) = tokio::io::duplex(1024);
            let answer = async {
                server.read_exact(&mut [0; 8]).await.unwrap();
                server.write_u8(b'S').await.unwrap();
            };
            let mut connection = tokio_postgres::Config::new();
            connection
                .user("test")
                .ssl_mode(tokio_postgres::config::SslMode::Require);
            let (connected, ()) = tokio::join!(
                connection.connect_raw(input, RejectedCertificate(certificate_error)),
                answer,
            );
            let error = DbError::database_connection(connected.err().unwrap());
            let (operation, cause, action) = error.postgres_diagnostic().unwrap();
            assert_eq!(operation, "PostgreSQL connection");
            assert!(cause.contains(expected), "{cause}");
            assert!(action.unwrap().contains(flag));
        }
    }

    #[tokio::test]
    async fn safe_diagnostic_keeps_sqlstate_without_driver_text() {
        let (input, mut server) = tokio::io::duplex(1024);
        let answer = async {
            let size = server.read_u32().await.unwrap() as usize;
            server.read_exact(&mut vec![0; size - 4]).await.unwrap();
            let detail = "relation params missing\u{1b}\u{202e} postgres://user:secret@db";
            let message = format!("SERROR\0VERROR\0C42P01\0M{detail}\0DSQL literal secret\0\0");
            server.write_u8(b'E').await.unwrap();
            server.write_u32((message.len() + 4) as u32).await.unwrap();
            server.write_all(message.as_bytes()).await.unwrap();
        };
        let mut connection = tokio_postgres::Config::new();
        connection
            .user("test")
            .ssl_mode(tokio_postgres::config::SslMode::Disable);
        let (connected, ()) =
            tokio::join!(connection.connect_raw(input, tokio_postgres::NoTls), answer);
        let error = DbError::postgres("PostgreSQL DBNames query", connected.err().unwrap());
        let (_, cause, action) = error.postgres_diagnostic().unwrap();
        assert_eq!(cause, "Relation \"params\" does not exist (SQLSTATE 42P01)");
        assert!(action.is_some());
        assert!(!error.to_string().contains("secret"));
        assert!(std::error::Error::source(&error).is_some());
    }
}
