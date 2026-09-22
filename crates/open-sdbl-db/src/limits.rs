//! Time and size limits a session applies to everything it does.

use std::time::Duration;

/// The limits one database session works under.
///
/// A session keeps the value it was opened with and applies it to every
/// call it makes. [`Limits::default`] yields what the `open-sdbl` console
/// uses: ten seconds to connect, two minutes for one server call, five
/// seconds for the PostgreSQL driver to wind down, 256 Config resources
/// decoded per round trip, and a million resources and two gibibytes of
/// compressed bytes per store a whole-configuration read collects.
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
    /// How many resources a whole-configuration read may retain from one
    /// store.
    ///
    /// The ceiling is applied to each store the read collects, not to the
    /// read as a whole: first to `Config`, then again, on its own, to the
    /// extension store `ConfigCAS`. Both sets are held at once, so the
    /// peak a read may reach is the ceiling once per store it collects,
    /// not once altogether.
    pub config_resource_limit: usize,
    /// How many compressed bytes a whole-configuration read may hold from
    /// one store.
    ///
    /// Applied per store, like [`Limits::config_resource_limit`], and
    /// with the same consequence for the peak.
    ///
    /// This bounds what the read *keeps*, which the decoding limits do
    /// not: those bound what is inflated at once. A store larger than
    /// this answers a typed error rather than exhausting the process.
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
