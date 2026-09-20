//! The secrets a session authenticates with.
//!
//! The carrier lives here; where a secret comes from — an environment
//! variable, a password file, a vault — is the application's policy.

use zeroize::Zeroizing;

use crate::error::DbError;

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

/// The secrets one session may need.
pub struct Credentials {
    /// The PostgreSQL password.
    pub postgres: EnvironmentSecret,
    /// The SQL Server password.
    pub mssql: EnvironmentSecret,
    /// The SOCKS5 proxy password.
    pub socks5: EnvironmentSecret,
}
