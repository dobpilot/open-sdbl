# Design: PostgreSQL wire-protocol gateway for SDBL

## Decisions taken with the maintainer (2026-10-26)

| # | Question | Decision |
|---|----------|----------|
| 1 | What "libpq adapter" means | A server speaking PostgreSQL protocol v3 (pgwire gateway). Not a drop-in `libpq.so` (would need FFI/`unsafe` and the libpq ABI), not a PostgreSQL extension (would require installing code on the 1C server, against the read-only model). |
| 2 | Backends | PostgreSQL and MSSQL 1C bases; the client always sees PostgreSQL. |
| 3 | Identity | Startup `user` is the information-base user; its roles, rights and restrictions apply. The gateway reaches the DBMS with its own operator-supplied credentials. |
| 4 | Protocol scope | Simple + Extended Query, TLS, CancelRequest. |
| 5 | Password check | Against the `v8users` hash; cleartext password message accepted only inside TLS. SCRAM is incompatible with a stored SHA-1 of the password and is not offered. |
| 6 | Placement | New library crate `open-sdbl-pgwire` with a hand-written codec + `open-sdbl serve` in the CLI. |
| 7 | Types | Native by `ColumnKind`, consistent with the 0.3.0 "no text casts" decision. |
| 8 | Non-SDBL chatter | Minimal local whitelist; no SQL pass-through ever. |

## Package boundary

```
psql / JDBC / DBeaver ──pgwire v3──▶ open-sdbl-pgwire ──▶ open-sdbl (compile)
                                           │
                                           └──▶ open-sdbl-db (DatabaseSession, access, cells) ──▶ PG / MSSQL
open-sdbl-cli: `serve` parses arguments, loads secrets and TLS files, builds
the config and calls open_sdbl_pgwire::serve(listener, config, shutdown).
```

- `open-sdbl` stays I/O-free and dependency-free. The two additions
  (parameter names, password check) are pure functions. SHA-1 is
  implemented in the core (≈80 lines, test vectors from FIPS 180-4) rather
  than pulling a crate; Base64 decoding of the stored hash is already needed
  nowhere else and is implemented locally too.
- `open-sdbl-pgwire` depends on `open-sdbl`, `open-sdbl-db`, `tokio`,
  `rustls`, `tokio-rustls` (already a transitive dependency through
  `tokio-postgres-rustls`), `zeroize`. It accepts an already-bound
  `TcpListener`, a `GatewayConfig` (connection description, `Credentials`,
  `Limits`, `ServerConfig` for TLS, operator session parameters, policy
  flags) and a shutdown future. It reads no files: certificates arrive as
  parsed rustls objects.
- `open-sdbl-cli` owns: argument parsing, reading the PEM files, passwords
  from env/`~/.pgpass` (existing mechanism), logging connection events to
  stderr, signal handling for graceful shutdown.

### Why a hand-written codec instead of the `pgwire` crate
The subset needed is small (≈20 message types), the framing is trivial, and
the state machine must be tightly bound to SDBL semantics (Parse cannot
compile, Describe needs a late compile, errors carry SDBL positions). The
`pgwire` crate brings its own abstractions, `async-trait`, extra TLS glue
and frequent breaking releases. A local codec is fuzzable with the existing
`fuzz/` workspace (new target `pgwire_frontend`).

## Shared state and per-connection state

- **Shared (one per process):** metadata snapshot and `QueryCompiler`
  (acquired once at start, refreshed on `SIGHUP` by the CLI / on an explicit
  call by library users), `AccessStore` with rights of all roles, user list
  from `v8users`, extension index. Wrapped in `Arc` and swapped atomically
  on refresh; connections in flight keep the old snapshot until their
  current statement ends.
- **Per client connection:** one `DatabaseSession` opened lazily on first
  query (pool is a later optimisation; one-to-one keeps transaction and
  cancellation semantics simple), the authenticated `InfoBaseUser`, its
  derived `RestrictionStore` and session parameters (computed exactly like
  console `\as`, at authentication), a `TempTablesManager`, named prepared
  statements and portals.

## Authentication flow

1. `SSLRequest` → `S` and TLS handshake when a TLS config is present, `N`
   otherwise. `GSSENCRequest` → `N`.
2. `StartupMessage`: protocol 3.0 required (3.2 negotiated down via
   `NegotiateProtocolVersion`); `user` required; `database` ignored but
   echoed; `application_name` kept for logs.
3. If the stream is not TLS and `allow_insecure_loopback` is not set for a
   loopback peer → `ErrorResponse 28000` without asking for a password.
4. `AuthenticationCleartextPassword`; the password is held in zeroized
   memory, checked with `UserData::verify_password` against the user found
   by exact (case-insensitive, as the platform does) name. Unknown user,
   wrong password, a user that cannot authenticate with a password
   (`standard authentication` off) → identical `28P01` message, constant
   response time padding to avoid user enumeration.
5. Restrictions are derived (see `query-repl` "Run allowed queries as a
   user"); derivation failures for single objects are logged, not fatal —
   those objects then compile to the expansion error at query time.
6. `AuthenticationOk`, `ParameterStatus` (`server_version` = `13.0
   (open-sdbl X.Y.Z)`, `server_encoding`/`client_encoding` = `UTF8`,
   `DateStyle` = `ISO, MDY`, `integer_datetimes` = `on`,
   `standard_conforming_strings` = `on`, `TimeZone` = `UTC`,
   `application_name`), `BackendKeyData` (random process id + 32-bit secret
   per connection), `ReadyForQuery I`.

A client asking for `client_encoding` other than UTF8 is refused at startup
(`22023`).

### Password hash — measurement first
The `v8users.Data` record carries the hash of the password and the hash of
its upper-cased form, Base64 of SHA-1. Their exact field positions and the
empty-password representation are **not yet measured** in this project.
Per the project's probe-base method, implementation starts by measuring on
probe bases of both layouts (8.2 and modern) with known passwords, and the
spec scenarios are fixed by those measurements. Until then the
requirement only says what the function must guarantee.

## Query flow

### Simple Query
Text → whitelist check → otherwise `prepare_with_options(text, Restricted)`
→ derived restrictions for the requested targets → `check_user_field_access`
→ `compile_batch` with session parameters and the connection
`TempTablesManager` → for each `CompiledQuery` of the batch that returns
rows: `RowDescription`, `DataRow`s streamed from `query_each`,
`CommandComplete SELECT n`; temp-table statements (`ПОМЕСТИТЬ`,
`УНИЧТОЖИТЬ`) answer `CommandComplete SELECT 0` / `DROP TABLE`. A text with
`&Имя` that has no value → error `42P02` naming the parameter. Then
`ReadyForQuery`.

### Extended Query and `$n` ↔ `&Имя`
- **Parse**: whitelist check, else `prepare`. Parameter slots are the names
  returned by `Prepared::parameter_names()` in first-occurrence order:
  `$1` is the first distinct `&Имя`, `$2` the second, and so on. A statement
  text containing literal `$n` tokens is an SDBL lexing error. The declared
  parameter OIDs of Parse are kept (0 = unspecified).
- **Describe statement**: `ParameterDescription` with the declared OIDs,
  unspecified → `text` (25); `RowDescription` requires a compile, so it is
  produced by compiling with every parameter set to SQL `NULL` (column kinds
  do not depend on values except for a bare `&Имя` projection, which is
  described as `text`). If that compile fails, `NoData` is sent and the
  error surfaces at Execute.
- **Bind**: decode each value by its OID and format into `ParameterValue`:
  `bool`→Boolean; `int2/int4/int8/numeric/float8`→Number (float8 only if it
  round-trips exactly); `text/varchar/bpchar/unknown`→String;
  `timestamp/timestamptz/date`→Date (timestamptz converted to UTC);
  `bytea`→16 bytes Reference, otherwise Binary; `uuid`→Uuid; one-dimensional
  arrays of the above→List (for `В`). NULL→Null. Anything else → `42804`.
  Then compile; compile diagnostics are reported here.
- **Execute** with `max_rows`: stream through `query_each`, suspend with
  `PortalSuspended` at the limit. Because the database layer reads inside
  one read-only transaction per call, a suspended portal keeps the remaining
  rows buffered up to `Limits`-based ceiling (new field
  `portal_buffer_rows`); exceeding it fails the portal with `54000` rather
  than re-running the query.
- **Sync** ends the implicit transaction; errors skip to Sync as the
  protocol requires.

### Transactions
`BEGIN`/`START TRANSACTION` open a gateway-side block: subsequent reads run
inside one `begin_read_only_transaction` so the client gets a consistent
snapshot; `COMMIT`/`END`/`ROLLBACK` close it. Status byte follows
(`I`/`T`/`E`). Anything that would write cannot be expressed in SDBL.

### Errors
`QueryDiagnostic` → `ErrorResponse` with `S ERROR`, `C` mapped from
`QueryDiagnosticKind` (syntax → `42601`, unknown name → `42703`/`42P01`,
missing parameter → `42P02`, restriction/rights → `42501`, others →
`XX000`), `M` the diagnostic message, `P` = 1-based **character** position
computed from the byte offset (the protocol counts characters), `H` hint
where the diagnostic has one. `DbError` → `08006` for a dead session,
`57014` for cancellation/timeout, `XX000` otherwise; the original DBMS
message is not forwarded verbatim in `M` when it may contain physical
table names, it goes to `D` (detail) only when the gateway runs with
`--verbose-errors`.

### Whitelist (exact match after trimming and case-folding)
| Client text | Answer |
|---|---|
| empty / only comments | `EmptyQueryResponse` |
| `SET name = value` / `SET name TO value` / `RESET …` | `CommandComplete SET`; `client_encoding` other than UTF8 → error; `application_name` stored and reported |
| `SHOW name` | one-row result with the startup `ParameterStatus` value, `transaction_isolation` = `read committed`, others → `42704` |
| `BEGIN`, `START TRANSACTION [READ ONLY]`, `COMMIT`, `END`, `ROLLBACK`, `ABORT` | see Transactions; `READ WRITE` → `25006` |
| `DISCARD ALL` | clears prepared statements, portals, temp tables |
| `SELECT version()` / `select current_schema()` / `SELECT 1` | fixed one-row answers |
Everything else is SDBL.

### Cancellation
`CancelRequest` on a new connection with a matching `(pid, secret)` calls
`DatabaseSession::cancel_query` with that connection's `QueryCancellation`;
the running statement ends with `57014`. Unknown keys are ignored silently
as PostgreSQL does.

### Type mapping (result columns)
| ColumnKind | OID | text format | binary format |
|---|---|---|---|
| Reference (16 / 20 bytes) | `bytea` 17 | `\x…` lower hex | raw bytes |
| Binary | `bytea` 17 | `\x…` | raw |
| String | `text` 25 (`varchar` 1043 with typmod when length known) | as is | UTF-8 |
| Number | `numeric` 1700 with precision/scale typmod | decimal text with scale | numeric binary |
| Boolean | `bool` 16 | `t`/`f` | 1 byte |
| DateTime | `timestamp` 1114 | `YYYY-MM-DD HH:MM:SS` | int64 µs since 2000-01-01 |
| Uuid | `uuid` 2950 | canonical lower case | 16 bytes |
| Type / Undefined / Null / Unknown | `text` 25 / `bytea` | as the console renders | — |
MSSQL year offset is already removed by the compiler, so dates are 1C
calendar dates for both backends. `numeric` binary encoding is implemented
from the decimal string; tests cross-check with `tokio-postgres` decoding.

## Security considerations
- Default listen address `127.0.0.1:5433`; binding a non-loopback address
  without TLS is refused by the CLI.
- No SQL ever reaches the DBMS except the compiler's SELECT output and the
  existing fixed acquisition statements.
- `RestrictionMode::Restricted` + complete field check: a target or field
  that cannot be proven readable refuses the statement before SQL is
  generated.
- Operator-bound session parameters (e.g. data separator values) are fixed
  per gateway; the client cannot set session parameters. This keeps tenant
  isolation in the operator's hands.
- Limits: maximum message size (default 16 MiB, `08P01` beyond), maximum
  concurrent connections, startup/authentication timeout, idle timeout,
  per-statement `query_timeout` from `Limits`.
- Failed authentication attempts are logged by the CLI with peer address
  and user name, never with the password.

## Testing strategy
- Codec unit tests per message, round-trip and malformed-frame tests; fuzz
  target `pgwire_frontend` in `fuzz/`.
- State-machine tests with an in-memory duplex stream and a fake executor
  trait (sealed inside the crate) — no database needed.
- Integration test using `tokio-postgres` as the client (dev-dependency)
  against the gateway with the fake executor: simple query, extended query
  with parameters of every OID class, describe, portal suspension, cancel,
  errors with positions, whitelist.
- Ignored live tests (like the MSSQL one) running `psql` against a real
  base on both backends.

## Open points to settle during implementation
- Exact `v8users` hash layout (measurement task, blocks the password
  requirement scenarios).
- Whether metadata refresh should also be triggered automatically when the
  configuration version in `Config` changes; first version: manual only.
