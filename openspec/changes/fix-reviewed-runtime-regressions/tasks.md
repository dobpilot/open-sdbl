## Implementation
- [x] Restore and test default extension decoding limits.
- [x] Guard MSSQL acquisition against cancellation during begin, reads and finish.
- [x] Share scalar equality rendering and cover source-free and binary operands.
- [x] Refuse lossy source-free logical projections with regression coverage.

## Validation
- [x] Run workspace formatting, Clippy with warnings denied, tests and rustdoc with warnings denied.
- [x] Run strict OpenSpec validation and document unavailable live-provider checks.

## Provider coverage
The cancellation regressions use a scripted loopback TDS peer with the real
Tiberius client: BEGIN, read, COMMIT, rollback and rollback verification are
interrupted deterministically. Success permits reuse, verified rollback preserves
the original error, and failed verification closes the connection. A live
SQL Server database is unavailable; live-provider tests remain opt-in.

## Results
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 1192 passed, 0 failed, 13 ignored; includes doctests.
- The recorded SQL corpus passed unchanged.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`: passed.
- `openspec validate --all --strict`: 47 items passed.
