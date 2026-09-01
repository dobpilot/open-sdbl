## Context

`MetadataSnapshot` already contains resolved objects, fields, authoritative
SchemaStorage declarations, and the live PostgreSQL catalog. The core query
module can build a snapshot-scoped `queryable_field_catalog` whose logical
fields retain their physical PostgreSQL members and types. Metadata acquisition
currently lives in `open-sdbl-cli` and uses asynchronous `tokio-postgres`
queries plus bounded `spawn_blocking` Config decoding.

Trino 476's stock Thrift connector invokes `trinoGetSplits` with desired
columns and a tuple domain. It does not implement `ConnectorMetadata.applyLimit`
and the published IDL has no limit member. Page-size requests are transport
limits, not SQL row limits. Using that connector would leave the required
PostgreSQL `LIMIT` pushdown impossible. The fallback architecture from the
request is therefore selected.

## Architecture

```text
CedrusData / Trino 476 coordinator and workers
        |
        | trino-open-sdbl (Java, trino-spi:476)
        | versioned HTTP + streaming NDJSON
        v
open-sdbl-trino (Rust)
        |-- MetadataSnapshot + queryable_field_catalog
        |-- metadata cache and safe SQL adapter
        |-- tokio-postgres connection pool
        v
1C PostgreSQL information base
```

The Java module does not parse SDBL, decode 1C metadata, or generate PostgreSQL
SQL. It converts Trino SPI handles and `Domain` values into a typed transport
request. The Rust service owns identifier resolution, SQL construction,
parameter binding, database access, and row conversion.

## Decisions

### Use a versioned HTTP protocol

The connector uses JDK HTTP clients and a streaming response so workers do not
buffer a table. Metadata responses and scan requests are JSON. Scan rows are
newline-delimited JSON with one bounded row per line. Protocol structures carry
stable logical names and typed literals; raw SQL is never accepted from Java.

### Keep one split for the MVP

The connector returns one real split containing the pushed table handle. This
does not claim parallelism. The handle carries schema, table, accepted domains,
selected columns, and the minimum pushed limit. Future split strategies can add
key/hash/range bounds without changing logical metadata.

### Translate only complete, safe domains

The Java connector pushes a column domain only when every range/value can be
encoded losslessly for that Trino type. Unsupported domains remain in Trino.
Rust validates every field and operator again, quotes identifiers internally,
and binds literals as PostgreSQL parameters. `NULL`, `NOT NULL`, equality,
inequality, ordered ranges, and discrete `IN` sets are supported.

### Deterministic metadata names

Every live, named tabular object maps to the Russian schema name of its actual
`MetadataKind` and to its Config object name. Case-folded duplicates receive a
stable GUID suffix and are reported as metadata issues. Nameless, non-live,
or unrepresentable objects are reported by the service instead of being
silently ignored.

### Type representations

Simple physical fields map to boolean, integer, bigint, decimal, varchar, date,
timestamp(3), UUID, or varbinary where the representation is lossless.
References are exposed as UUID when the logical field has one RRef payload and
the target type is fixed. A multi-target reference is encoded as a documented
varchar containing its type discriminator and UUID. Other compound 1C values
are exposed as JSON containing all physical members, so no member is silently
dropped; filter pushdown is not claimed for those JSON columns.

### Cache immutable generations

The service publishes `Arc`-owned metadata generations. One task refreshes an
expired generation while readers continue using the last valid generation.
Startup readiness requires one successful load. An authenticated administrative
refresh is desirable but not required for the first vertical slice; a local
refresh endpoint can be disabled by default.

### Read-only database safety

Metadata loads and scans use explicit read-only transactions. Query timeout,
PostgreSQL statement timeout, pool size, response batch size, and an optional
maximum result safeguard are configured independently. A configured maximum
never changes SQL semantics silently: requests exceeding it fail explicitly.

## Error handling

The Rust API returns stable error codes for missing object/column, unsupported
type/domain, invalid metadata, PostgreSQL connection/query failures, compiler
or adapter failures, timeout, and internal protocol errors. The Java connector
maps them to Trino exceptions without exposing Rust backtraces. Passwords and
database URLs are redacted from logs.

## Compatibility

The Java dependency is pinned exactly to `io.trino:trino-spi:476`. The plugin
uses only upstream SPI. CedrusData 476-5 is treated as a Trino 476-compatible
distribution; deployment documentation includes an installation smoke test
because no public CedrusData source proves binary identity of every build.
