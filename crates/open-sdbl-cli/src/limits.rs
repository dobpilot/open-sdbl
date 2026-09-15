//! Time and size limits every layer of the CLI shares.
//!
//! They live in a module of their own so that the database layer does not
//! import them from the binary root: a lower layer must not depend on the
//! binary that drives it.

use std::time::Duration;

/// How long a connection attempt may take before it is abandoned.
pub(crate) const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
/// How long one server call may run before the CLI stops waiting.
pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(120);
/// How long a PostgreSQL driver may take to wind down on close.
pub(crate) const POSTGRES_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
/// How many Config resources are decoded per round trip.
pub(crate) const CONFIG_DECODE_BATCH_SIZE: usize = 256;
/// The statement that reads the transaction depth of an MSSQL session.
pub(crate) const MSSQL_TRANSACTION_COUNT: &str = "SELECT CONVERT(int, @@TRANCOUNT)";
