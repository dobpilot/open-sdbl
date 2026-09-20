# Design — streaming a query result

## Context

Both providers already stream *metadata*: `transaction.query_raw(…)`
yields a `RowStream` in `db/postgres/metadata.rs`, and
`simple_query(…).into_row_stream()` yields a row stream in
`db/mssql/metadata.rs`. Only the user-query path collects.

## Decisions

### 1. A callback, not a returned stream

A returned stream would have to keep the transaction alive, and the
transaction borrows the client that lives in the session. Handing that out
means either a self-referential value — which this crate will not write
without `unsafe` — or a guard type whose `Drop` cannot `await` the
rollback it owes. Both are more machinery than the requirement needs.

The requirement is that the consumer can *stop*. A callback gives exactly
that:

```rust
pub enum RowFlow { Continue, Stop }

pub async fn query_each(
    &mut self,
    sql: &str,
    column_count: usize,
    on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
) -> Result<(), DbError>;
```

The session keeps the transaction, drives the provider stream, decodes one
row, hands it over, and stops when told. `query` becomes

```rust
let mut rows = Vec::new();
self.query_each(sql, columns, |row| { rows.push(row); Ok(RowFlow::Continue) }).await?;
```

so its signature, its errors, and its transaction handling are unchanged
by construction rather than by inspection.

`RowFlow` is `#[non_exhaustive]`: a later "skip the rest but finish the
transaction" is then additive.

### 2. The loop is testable without a database

The acceptance asks to *show* that stopping allocates nothing for the
remainder, which a live test cannot show. So the loop is one provider-free
function over a `Stream` of decoded rows:

```rust
async fn drive_rows<S>(rows: S, on_row: …) -> Result<Stopped, DbError>
where S: Stream<Item = Result<Vec<Cell>, DbError>>
```

A unit test drives it with a source that counts how many items were pulled
and panics if polled after the stop. That is evidence, not argument: the
rows after the stop are never produced, so nothing allocates them.

Each provider supplies the stream and keeps the transaction discipline it
already has.

### 3. Stopping is how SQL Server cancels

Dropping a Tiberius query future leaves the TDS stream between protocol
messages — `cancel_and_reconnect` already says so and poisons the session
rather than sending cleanup on it. Stopping a read is the same situation,
so it takes the same answer: the client is dropped, the session is
poisoned, and the caller reconnects.

That is not a workaround; it is the only cancellation SQL Server has here,
because this crate sets `LOCK_TIMEOUT` and no execution limit. The
specification states it so that a consumer knows a stopped SQL Server read
costs the connection.

PostgreSQL needs none of this: the transaction is rolled back and the
session stays usable, because `statement_timeout` already bounds the
statement server-side.

### 4. What is not in this change

No row or byte budget is imposed. A budget belongs to the consumer that
knows what it can hold; `query_each` is what lets it have one at all.
Adding a cap to `query` would change the behaviour this change promises to
keep.

## No new dependencies

`futures-util` is already a dependency of `open-sdbl-db`; the core crate
is not touched.
