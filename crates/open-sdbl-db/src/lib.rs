//! The database layer of `open-sdbl`.
//!
//! This crate owns everything between an information base and the
//! deterministic core: opening a session against PostgreSQL or Microsoft
//! SQL Server in a verified read-only transaction, reading and resolving
//! the 1C metadata of the base, decoding the values a query returns, and
//! answering what a user is allowed to see — the users of the base, the
//! rights of their roles, the templates those rights reference, and the
//! access restrictions the roles expand into.
//!
//! It performs database and network I/O and nothing else: it writes
//! nothing to a terminal, parses no command line, and reads no
//! environment variable or password file. The application supplies the
//! connection description ([`DatabaseConnection`]), the secrets
//! ([`Credentials`]), the limits ([`Limits`]) and, if it wants one, a
//! progress reporter ([`MetadataProgress`]).
//!
//! ```no_run
//! # async fn example(connection: &open_sdbl_db::DatabaseConnection,
//! #                  credentials: &open_sdbl_db::Credentials)
//! # -> Result<(), open_sdbl_db::DbError> {
//! use open_sdbl_db::{DatabaseSession, Limits, NoProgress};
//!
//! let mut session =
//!     DatabaseSession::connect(connection, credentials, Limits::default()).await?;
//! let (snapshot, _report) = session.metadata(&mut NoProgress).await?;
//! let rows = session.query("SELECT 1", 1).await?;
//! session.close().await?;
//! # let _ = (snapshot, rows);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod access;
pub mod access_cache;
pub mod cells;
pub mod connection;
pub mod credentials;
pub mod db;
pub mod error;
pub mod extensions;
pub mod limits;
pub mod net;
pub mod pipeline;
pub mod progress;
pub mod restrict;
mod rows;
pub mod session;

pub use access::{
    AccessStore, DerivedRestrictions, derive_restrictions, describe_role, describe_templates,
    describe_user, ensure_rights, ensure_users, list_restrictions, list_roles, list_users,
    rls_report, role_restrictions, user_decisions, user_restrictions,
};
pub use access_cache::{TemplateParameters, read_current_user, read_template_parameters};
pub use cells::{Cell, DateTimeParts, QueryRows, RowFlow};
pub use connection::{
    ConnectionOptions, DatabaseConnection, MsSqlConnection, PostgresConnection, PostgresSslMode,
};
pub use credentials::{Credentials, EnvironmentSecret};
pub use db::mssql::{MsSqlSession, decode_mssql_cell, mssql_row};
pub use db::postgres::{PostgresCell, PostgresSession};
pub use error::DbError;
pub use extensions::{ExtensionIndex, read_extension_index, read_extension_roles};
pub use limits::Limits;
pub use net::socks5::{Socks5Proxy, parse_socks5_proxy};
pub use pipeline::{
    AcquiredConfiguration, AcquiredExtension, ConfigResource, ExtensionCatalogRow, MetadataSource,
    acquire_configuration, acquire_metadata,
};
pub use progress::{MetadataProgress, NoProgress};
pub use restrict::{RestrictionOrigin, RestrictionStore};
pub use session::{
    DatabaseDialect, DatabaseSession, QueryCancellation, bounded_database_call, query_timeout,
};

/// The hex decoder the library tests use, shared so that a fixture written
/// as hex reads the same way on both sides.
#[cfg(test)]
#[path = "../../../tests/support/hex.rs"]
mod hex_test_support;

/// The metadata snapshot the parameter and restriction tests resolve.
#[cfg(test)]
#[path = "../../../tests/support/enumeration_snapshot.rs"]
mod enumeration_snapshot_support;
