//! Tests of the `rows` module.
//!
//! The point of the streaming path is that a stop costs nothing for the
//! remainder. A source that panics when polled after the stop proves it:
//! the rows that follow are never produced, so nothing allocates them.

use std::cell::Cell as StdCell;

use futures_util::stream;

use super::{ReadEnd, drive_rows};
use crate::cells::{Cell, RowFlow};
use crate::error::DbError;

/// A row source that records how many rows were pulled out of it and
/// refuses to be polled after the caller said stop.
fn counting_source(
    rows: usize,
    polled: &StdCell<usize>,
) -> impl futures_util::Stream<Item = Result<Vec<Cell>, DbError>> + '_ {
    stream::iter(0..rows).map(move |index| {
        polled.set(polled.get() + 1);
        Ok(vec![Cell::Number(index.to_string())])
    })
}

use futures_util::StreamExt as _;

#[tokio::test]
async fn stopping_reads_nothing_further() {
    let polled = StdCell::new(0);
    let mut seen = Vec::new();
    let end = drive_rows(counting_source(1_000_000, &polled), |row| {
        seen.push(row);
        Ok(if seen.len() == 10 {
            RowFlow::Stop
        } else {
            RowFlow::Continue
        })
    })
    .await
    .unwrap();

    assert_eq!(end, ReadEnd::Stopped);
    assert_eq!(seen.len(), 10, "exactly the rows the caller asked for");
    assert_eq!(
        polled.get(),
        10,
        "the source was never pulled past the stop, so the remaining \
         999_990 rows were never produced or allocated"
    );
}

#[tokio::test]
async fn reading_to_the_end_sees_every_row_in_order() {
    let polled = StdCell::new(0);
    let mut seen = Vec::new();
    let end = drive_rows(counting_source(5, &polled), |row| {
        seen.push(row);
        Ok(RowFlow::Continue)
    })
    .await
    .unwrap();

    assert_eq!(end, ReadEnd::Exhausted);
    assert_eq!(polled.get(), 5);
    let labels = seen
        .iter()
        .map(|row| row[0].render().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(labels, ["0", "1", "2", "3", "4"]);
}

#[tokio::test]
async fn an_error_from_the_caller_ends_the_read() {
    let polled = StdCell::new(0);
    let error = drive_rows(counting_source(100, &polled), |_| {
        Err(DbError::Data("the caller refused the row".to_owned()))
    })
    .await
    .unwrap_err();

    assert!(error.to_string().contains("refused"));
    assert_eq!(polled.get(), 1, "the read stopped at the failing row");
}

#[tokio::test]
async fn an_error_from_the_source_ends_the_read() {
    let rows = stream::iter([
        Ok(vec![Cell::Null]),
        Err(DbError::Data("the server hung up".to_owned())),
        Ok(vec![Cell::Null]),
    ]);
    let mut seen = 0;
    let error = drive_rows(rows, |_| {
        seen += 1;
        Ok(RowFlow::Continue)
    })
    .await
    .unwrap_err();

    assert!(error.to_string().contains("hung up"));
    assert_eq!(seen, 1);
}
