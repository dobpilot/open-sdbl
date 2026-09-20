//! Time and size limits a session applies to everything it does.

use std::time::Duration;

/// The limits one database session works under.
///
/// A session keeps the value it was opened with and applies it to every
/// call it makes. [`Limits::default`] yields what the `open-sdbl` console
/// uses: ten seconds to connect, two minutes for one server call, five
/// seconds for the PostgreSQL driver to wind down, and 256 Config
/// resources decoded per round trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How long a connection attempt may take before it is abandoned.
    pub connection_timeout: Duration,
    /// How long one server call may run before the caller stops waiting.
    pub query_timeout: Duration,
    /// How long a PostgreSQL driver may take to wind down on close.
    pub postgres_close_timeout: Duration,
    /// How many Config resources are decoded per round trip.
    pub config_decode_batch_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(120),
            postgres_close_timeout: Duration::from_secs(5),
            config_decode_batch_size: 256,
        }
    }
}
