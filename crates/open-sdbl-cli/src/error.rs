use std::fmt;
use std::io;

use open_sdbl::Diagnostic;
use open_sdbl::metadata::MetadataError;

#[derive(Debug)]
pub(crate) enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
    Metadata(MetadataError),
    Data(String),
    Database(String),
    MsSql {
        operation: &'static str,
        source: tiberius::error::Error,
    },
    Terminal(String),
}

impl CliError {
    pub(crate) const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) | Self::Metadata(_) | Self::Data(_) => 1,
            Self::Usage(_)
            | Self::Io(_, _)
            | Self::Database(_)
            | Self::MsSql { .. }
            | Self::Terminal(_) => 2,
        }
    }

    pub(crate) fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL connection failed: {error}"))
    }

    pub(crate) fn mssql_connection(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "connection",
            source: error,
        }
    }

    pub(crate) fn mssql_query(error: tiberius::error::Error) -> Self {
        Self::MsSql {
            operation: "query",
            source: error,
        }
    }

    pub(crate) fn socks5_connection(error: impl fmt::Display) -> Self {
        Self::Database(format!("SOCKS5 proxy connection failed: {error}"))
    }

    pub(crate) fn standard_output(error: io::Error) -> Self {
        Self::Io("cannot write standard output".to_owned(), error)
    }

    pub(crate) fn is_broken_pipe(&self) -> bool {
        matches!(self, Self::Io(_, error) if error.kind() == io::ErrorKind::BrokenPipe)
    }

    pub(crate) fn is_mssql_connection_failure(&self) -> bool {
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
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
            Self::MsSql { operation, source } => {
                write!(formatter, "MSSQL {operation} failed: {source}")
            }
            Self::Data(message) | Self::Database(message) | Self::Terminal(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl From<MetadataError> for CliError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<tokio_postgres::Error> for CliError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL query failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::CliError;

    #[test]
    fn recognizes_broken_standard_output() {
        let error = CliError::standard_output(io::Error::from(io::ErrorKind::BrokenPipe));
        assert!(error.is_broken_pipe());
    }
}
