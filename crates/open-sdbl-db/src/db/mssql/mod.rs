//! The SQL Server provider: a session, the metadata it reads and the
//! decoding of the values it returns.

mod cells;
mod metadata;
mod session;

pub use cells::{decode_mssql_cell, mssql_row};
pub use session::MsSqlSession;
