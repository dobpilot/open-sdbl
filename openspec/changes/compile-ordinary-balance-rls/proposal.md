# Compile ordinary balance RLS checks

## Why
Measured native ordinary accumulation Balance queries return permitted rows and fail when a selected balance is forbidden; filtering alone cannot distinguish these outcomes. The SDK currently exposes no execution contract for that distinction. Evidence is retained in `tests/fixtures/access/native-0910/query-rls-allowed.platform.txt`, `query-rls-queries.platform.txt`, and `query-rls-manager-filter.platform.txt`.

## What Changes
- Add an opt-in compilation option and a structured forbidden-row service-column index.
- Compile supported ordinary Balance reads and their forbidden-row witness into one SELECT-only PostgreSQL statement.
- Preserve ALLOWED filtering and refuse unsupported ordinary shapes explicitly.
- Bump workspace packages to 0.9.21.

## Impact
Public Rust compiler API and generated SQL change only under the explicit option. No dependency or database I/O enters the library. Hosts must inspect all service markers before decoding or publishing rows.
