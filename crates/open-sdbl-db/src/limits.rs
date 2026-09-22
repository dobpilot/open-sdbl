//! Time and size limits a session applies to everything it does.

use std::time::Duration;

/// The limits one database session works under.
///
/// A session keeps the value it was opened with and applies it to every
/// call it makes. [`Limits::default`] yields what the `open-sdbl` console
/// uses: ten seconds to connect, two minutes for one server call, five
/// seconds for the PostgreSQL driver to wind down, 256 Config resources
/// decoded per round trip, and a whole-configuration read bounded at a
/// million resources and two gibibytes of compressed bytes.
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
    /// How many `Config` resources a whole-configuration read may retain.
    pub config_resource_limit: usize,
    /// How many compressed bytes a whole-configuration read may hold.
    ///
    /// This bounds what the read *keeps*, which the decoding limits do
    /// not: those bound what is inflated at once. A base larger than this
    /// answers a typed error rather than exhausting the process.
    pub config_retained_byte_limit: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(120),
            postgres_close_timeout: Duration::from_secs(5),
            config_decode_batch_size: 256,
            config_resource_limit: 1_000_000,
            config_retained_byte_limit: 2 * 1024 * 1024 * 1024,
        }
    }
}
