# Add a PostgreSQL wire-protocol gateway that accepts SDBL

## Why
Today SDBL can be run only from the `open-sdbl console` REPL or by embedding
the libraries. Any tool that speaks libpq — `psql`, DBeaver, JDBC/ODBC
drivers, BI tools, Python `psycopg`, the Trino connector — could read a 1C
base directly if something on the other end of a PostgreSQL connection
accepted SDBL text instead of SQL. A gateway that speaks the PostgreSQL
frontend/backend protocol v3, authenticates the client as an
information-base user, compiles each query against that user's rights, and
streams the rows back turns every libpq client into an SDBL client without
any client-side change.

## What Changes
- New library package `crates/open-sdbl-pgwire` (`open_sdbl_pgwire`): a
  hand-written protocol v3 codec and a connection state machine over tokio
  and rustls, which are already in the dependency tree. It owns the listening
  socket; it reads no environment, files or command line and writes nothing
  to a terminal.
- New CLI command `open-sdbl serve postgres|mssql`: the same connection
  options as `console`, plus listen address, TLS certificate and key, and
  operator-bound session parameters. Both 1C storage backends are served;
  the client always sees the PostgreSQL protocol.
- Protocol scope: SSLRequest/TLS, startup, cleartext password over TLS only
  (loopback without TLS only behind an explicit flag), Simple Query,
  Extended Query (Parse/Bind/Describe/Execute/Sync/Close/Flush), portals
  with row limits, CancelRequest, Terminate. No COPY, no replication, no
  GSSAPI, no SCRAM.
- Authentication: the startup `user` is the information-base user name; the
  password is verified against a confirmed `StoredPasswordValue`
  associated with that `v8users` user: legacy SHA-1 (`H1,H2`), or the
  supplied 8.3.27.2342 reference's four-field SHA-256 (ID 1), SHA-512
  (ID 2), and PBKDF2-HMAC-SHA256 (ID 3) profiles.
  The supplied Python verifier v0.2 defines the algorithm: H1 authorizes,
  H2 is diagnostic only, and unsupported or unconfirmed values fail closed.
  Every query is compiled in `RestrictionMode::Restricted` with that user's derived
  restrictions and the complete field-rights check, failing closed.
- Results: columns are described natively from `ColumnKind` (reference →
  `bytea`, string → `text`, number → `numeric`, boolean → `bool`, date →
  `timestamp`, UUID → `uuid`), text and binary result formats.
- A fixed whitelist of client chatter (`SET`, `SHOW`, `BEGIN`/`COMMIT`/
  `ROLLBACK`, `DISCARD`, `SELECT version()`, `current_schema()`, empty
  query) is answered locally; every other text is compiled as SDBL. Raw SQL
  is never forwarded to the database.
- Core library additions (no I/O, no dependencies): `Prepared` exposes the
  query parameter names in first-occurrence order so Extended Query `$n`
  slots can be bound to `&Имя`; a typed stored-password verifier and
  `UserData::verify_password` adapter check these four algorithms without
  exposing hashes. Extended records append literal UTF-8 salt to the
  password; PBKDF2 uses that concatenation as its password, an empty
  separate salt, 16,000 iterations, and 64 derived bytes.
  The verifier consumes the public `StoredPasswordValue` property; the
  adapter may use only an already-confirmed source mapping and otherwise
  reports an indeterminate result. This change does not discover raw
  `v8users.Data` password fields.

## Impact
- Specs: new capability `pgwire-gateway`; deltas to `crate-architecture`,
  `query-compilation`, `access-rights`.
- Code: new workspace member, new CLI subcommand, public additions
  in `open-sdbl`, no change to `open-sdbl-db` behaviour beyond reuse.
- No new third-party production dependency (see design).
- Version bump of all workspace packages to the next minor (0.10.0): new
  package and new CLI command.

## Non-goals
- Emulating `pg_catalog`/`information_schema` so that GUI trees show 1C
  objects — a separate change.
- Writing data, COPY, LISTEN/NOTIFY, logical replication.
- Forwarding SQL to the database, or any pass-through mode.
- SCRAM-SHA-256 or proxy-maintained password files.
- Operating-system (Windows) authentication of 1C users.
- Password algorithms outside the supplied legacy and ID 1–3 profiles;
  four-field ID 0 and unknown IDs are unsupported, without algorithm fallback.
- Reverse engineering, probe-base password measurements, raw `v8users.Data`
  password-field discovery, or inferring a source profile from hash-shaped
  text. Synthetic verification vectors do not establish a raw-record layout
  or prove successful live 1C authentication.
