//! Driving a decoded row stream into a caller that may stop at any point.
//!
//! The loop lives here, free of any provider, so that what it promises —
//! that a stop reads nothing further — can be tested without a database.

use futures_util::{Stream, StreamExt};

use crate::cells::{Cell, RowFlow};
use crate::error::DbError;

/// Whether the caller stopped before the result ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadEnd {
    /// The result was read to the end.
    Exhausted,
    /// The caller answered [`RowFlow::Stop`].
    Stopped,
}

/// Hands every row of `rows` to `on_row` until it stops or the stream ends.
///
/// The stream is dropped as soon as the caller stops, so a provider that
/// holds a server-side cursor stops fetching there.
pub(crate) async fn drive_rows<S, F>(rows: S, mut on_row: F) -> Result<ReadEnd, DbError>
where
    S: Stream<Item = Result<Vec<Cell>, DbError>>,
    F: FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
{
    let mut rows = std::pin::pin!(rows);
    while let Some(row) = rows.next().await {
        match on_row(row?)? {
            RowFlow::Continue => {}
            RowFlow::Stop => return Ok(ReadEnd::Stopped),
        }
    }
    Ok(ReadEnd::Exhausted)
}

#[cfg(test)]
#[path = "tests/rows.rs"]
mod tests;
