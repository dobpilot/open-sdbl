//! The secrets a session authenticates with.
//!
//! The carrier lives here; where a secret comes from — an environment
//! variable, a password file, a vault — is the application's policy.

use std::fmt;

use zeroize::Zeroizing;

use crate::error::DbError;

#[cfg(test)]
#[path = "tests/credentials.rs"]
mod tests;

/// One secret an application supplies, or the reason it could not.
#[derive(Clone)]
#[non_exhaustive]
pub enum EnvironmentSecret {
    /// The application has no secret of this kind.
    Missing,
    /// A secret was present but is not valid UTF-8.
    InvalidUnicode,
    /// The secret, held in memory that is wiped when it is dropped.
    Present(Zeroizing<String>),
}

impl EnvironmentSecret {
    /// The secret `secret` holds, or [`EnvironmentSecret::Missing`].
    ///
    /// The string is moved, not copied, into memory that is wiped when it
    /// is dropped, so a caller hands over its only copy and never names
    /// the type that wipes it.
    #[must_use]
    pub fn from_option(secret: Option<String>) -> Self {
        secret.map_or(Self::Missing, |secret| {
            Self::Present(Zeroizing::new(secret))
        })
    }

    /// The secret, when there is one; an error when it is unreadable.
    ///
    /// `name` names the secret in the error, so the operator learns which
    /// variable to fix.
    pub fn optional(&self, name: &str) -> Result<Option<Zeroizing<String>>, DbError> {
        match self {
            Self::Missing => Ok(None),
            Self::InvalidUnicode => Err(DbError::Data(format!("{name} is not valid UTF-8"))),
            Self::Present(value) => Ok(Some(value.clone())),
        }
    }

    /// The secret, or an error naming it when there is none.
    pub fn required(&self, name: &str) -> Result<Zeroizing<String>, DbError> {
        self.optional(name)?.ok_or_else(|| {
            DbError::Data(format!(
                "{name} is required for the selected authentication method"
            ))
        })
    }
}

impl fmt::Debug for EnvironmentSecret {
    /// Says whether there is a secret; never what it is.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "Missing",
            Self::InvalidUnicode => "InvalidUnicode",
            Self::Present(_) => "Present(<redacted>)",
        })
    }
}

/// The secrets one session may need.
pub struct Credentials {
    /// The PostgreSQL password.
    pub postgres: EnvironmentSecret,
    /// The SQL Server password.
    pub mssql: EnvironmentSecret,
    /// The SOCKS5 proxy password.
    pub socks5: EnvironmentSecret,
}

impl Credentials {
    /// The secrets of a session, from strings the caller owns.
    ///
    /// Each string is moved into memory that is wiped when it is dropped;
    /// `None` is a secret the caller does not have. Which of them a
    /// session needs depends on the connection it opens.
    #[must_use]
    pub fn new(postgres: Option<String>, mssql: Option<String>, socks5: Option<String>) -> Self {
        Self {
            postgres: EnvironmentSecret::from_option(postgres),
            mssql: EnvironmentSecret::from_option(mssql),
            socks5: EnvironmentSecret::from_option(socks5),
        }
    }
}

impl fmt::Debug for Credentials {
    /// Says which secrets are present; never what they are.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("postgres", &self.postgres)
            .field("mssql", &self.mssql)
            .field("socks5", &self.socks5)
            .finish()
    }
}
