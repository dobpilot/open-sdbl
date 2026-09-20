//! The PostgreSQL provider: a session, the metadata it reads and the
//! decoding of the values it returns.

mod cells;
mod metadata;
mod session;

pub use cells::PostgresCell;
pub use session::PostgresSession;
/// Raw connection helpers the SOCKS5 tests drive directly.
#[cfg(test)]
pub(crate) use session::connect_postgres_raw;
