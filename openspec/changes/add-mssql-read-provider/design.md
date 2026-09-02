# Design: Microsoft SQL Server read provider

## Context

The core parser and resolver are deterministic, but their SQL renderer and the
CLI session are named and shaped around PostgreSQL. SQL Server stores the same
1C resources and physical object names, while differing in catalog APIs,
literal syntax, text conversion, result limiting, and connection protocol.

SQL Server does not expose a per-transaction read-only mode equivalent to
PostgreSQL. The connector can request `ApplicationIntent=ReadOnly` and execute
only compiler-produced/fixed SELECT statements, but deployment must still use
a SQL login granted only the required SELECT permissions for defense in depth.

## Decisions

### Keep one parser and resolver

The core gains an internal dialect selection and public MSSQL compile/prepare
entry points. PostgreSQL APIs remain wrappers selecting the existing dialect.
No parser, AST, metadata decoder, or presentation policy is duplicated.

### Render native T-SQL

The MSSQL dialect emits native T-SQL at generation time. In particular it uses
`TOP`, `CONVERT(nvarchar(max), ...)`, Unicode string literals, and `0x...`
binary literals. PostgreSQL SQL is not post-processed with textual replacement.
Unsupported dialect constructs fail before database execution.

### Isolate TDS in the CLI crate

`open-sdbl-cli` uses Tiberius over Tokio TCP, optionally through the existing
SOCKS5 transport. The core crate remains free of production dependencies and
I/O. A small session enum gives the REPL common metadata/query operations and
owned textual rows, avoiding leakage of either driver's row type.

### Acquire authoritative metadata with fixed SELECTs

SQL Server queries read `Params.BinaryData`, GUID-named `Config` resources,
`SchemaStorage.CurrentSchema`, and `sys.tables`/`sys.columns`/`sys.indexes`.
Every query is fixed in the core, SELECT-only, and scoped to `dbo`. Existing
decoders and deterministic resolution consume the returned owned values.

The adapter also reads the single `dbo._YearOffset.Offset` value. The MSSQL
renderer subtracts this value from projected datetime columns and adds it to
logical date literals used in predicates and virtual-table boundaries. This
keeps logical 1C dates stable for databases using either offset 0 or 2000.

### Read configuration-extension table variants as one relation

1C can redirect rows of an extended object from its canonical table, such as
`_Reference18`, into one or more structurally compatible tables named with an
`X` suffix, such as `_Reference18X1`. The MSSQL renderer treats the canonical
table and its exact `X[digits]` variants as one `UNION ALL` relation. It exposes
the canonical field set, substitutes `NULL` when a variant lacks a canonical
column, and ignores variant-only columns unknown to the resolved metadata.
This relation is reused for direct object reads, dereference joins, and
presentation joins. Unrelated tables that merely share a textual prefix are
not included.

### Secure defaults

TLS certificate validation remains enabled by default. The CLI accepts
`--trust-server-certificate` only as an explicit flag. SQL authentication reads
its secret from `MSSQL_PASSWORD`; secrets are not accepted as positional values
and never appear in diagnostics. The TDS login requests read-only application
intent. Documentation requires a SELECT-only SQL login.

## Non-goals

- Windows integrated authentication, Kerberos, SQL Browser named instances,
  and Azure AD authentication are not part of this change.
- The provider does not write 1C data or metadata.
- The provider does not promise that `ApplicationIntent=ReadOnly` is an access
  control boundary.
