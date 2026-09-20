use std::fmt;
use std::io;

use open_sdbl::Diagnostic;
use open_sdbl::metadata::MetadataError;
use open_sdbl_db::DbError;

#[derive(Debug)]
pub(crate) enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
    Metadata(MetadataError),
    Data(String),
    PostgresPlaintextOptInRequired,
    Terminal(String),
    /// Whatever the database layer reported, kept as it is.
    Db(DbError),
}

impl CliError {
    pub(crate) const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) | Self::Metadata(_) | Self::Data(_) => 1,
            // The database layer reports the same two kinds the flat enum
            // did, so the exit status of a run does not change.
            Self::Db(DbError::Metadata(_) | DbError::Data(_)) => 1,
            Self::Usage(_)
            | Self::Io(_, _)
            | Self::PostgresPlaintextOptInRequired
            | Self::Terminal(_)
            | Self::Db(_) => 2,
        }
    }

    pub(crate) fn standard_output(error: io::Error) -> Self {
        Self::Io("cannot write standard output".to_owned(), error)
    }

    pub(crate) fn is_broken_pipe(&self) -> bool {
        match self {
            Self::Io(_, error) | Self::Db(DbError::Io(_, error)) => {
                error.kind() == io::ErrorKind::BrokenPipe
            }
            _ => false,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
            Self::Db(error) => error.fmt(formatter),
            Self::PostgresPlaintextOptInRequired => formatter.write_str(
                "error[OPEN_SDBL_CLI_PG_PLAINTEXT_OPT_IN_REQUIRED]: plaintext PostgreSQL transport requires explicit confirmation\n\
cause: --sslmode disable turns off encryption and certificate verification\n\
help: add --insecure-plaintext to accept plaintext, or remove --sslmode disable to use verify-full\n\
note: --socks5-proxy routes traffic but does not provide PostgreSQL transport security",
            ),
            Self::Data(message) | Self::Terminal(message) => formatter.write_str(message),
        }
    }
}

impl From<MetadataError> for CliError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<DbError> for CliError {
    fn from(error: DbError) -> Self {
        Self::Db(error)
    }
}

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
