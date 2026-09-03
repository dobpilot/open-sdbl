//! PostgreSQL backend for the generic query compiler.

use super::core::SqlDialect;
use super::sealed;

/// Stateless PostgreSQL backend value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PostgresBackend;

impl sealed::Sealed for PostgresBackend {
    fn dialect(self) -> SqlDialect {
        SqlDialect::Postgres
    }
}
