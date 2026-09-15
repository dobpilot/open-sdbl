//! The SQL Server provider: a session, the metadata it reads and the
//! decoding of the values it returns.

mod cells;
mod metadata;
mod session;

pub(crate) use session::MsSqlSession;
