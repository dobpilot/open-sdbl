## 1. The loop

- [x] 1.1 `RowFlow` and a provider-free `drive_rows` over a stream of
  decoded rows.
- [x] 1.2 A unit test whose source panics when polled after a stop, and
  which asserts how many rows were pulled.

## 2. The providers

- [x] 2.1 PostgreSQL `query_each` over `query_raw`, decoding one row at a
  time, keeping the read-only transaction discipline.
- [x] 2.2 SQL Server `query_each` over `into_row_stream`; a stop drops the
  client and poisons the session.
- [x] 2.3 `DatabaseSession::query_each`, and `query` defined in terms of
  it.

## 3. Tests

- [x] 3.1 Reading to the end yields the same rows as `query`.
- [x] 3.2 Stopping decodes exactly the rows asked for.
- [x] 3.3 A live SQL Server test: stopping early leaves the session
  unusable, and a reconnect works.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `cargo build --release --locked`
- [x] 4.6 `openspec validate stream-query-results --strict`; archive
