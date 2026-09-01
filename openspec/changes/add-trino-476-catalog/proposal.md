## Why

CedrusData 476-5 needs to query resolved 1C objects alongside other Trino
catalogs. The existing CLI can load and query 1C PostgreSQL metadata, but it
does not expose a Trino connector contract and loading all rows before
filtering would be unsafe for production databases.

The stock Trino 476 Thrift connector was evaluated first. Its service contract
transports projections and tuple-domain constraints, but it has no operation or
handle field for `LIMIT`. It therefore cannot satisfy the required remote limit
pushdown. Its wire blocks also omit decimal, UUID, and varbinary. A small Trino
SPI 476 adapter backed by a Rust HTTP service is required for the complete MVP.

## What Changes

- Add an `open-sdbl-trino` Rust service that loads and caches 1C metadata,
  exposes deterministic Trino metadata, and executes parameterized read-only
  PostgreSQL scans.
- Add a thin Java `trino-open-sdbl` connector compiled against
  `io.trino:trino-spi:476`; it delegates metadata and scans to the Rust service
  and implements filter, projection, and limit pushdown.
- Add explicit 1C/PostgreSQL-to-Trino type mappings, structured diagnostics,
  health/readiness endpoints, bounded query execution, and lightweight
  metrics.
- Add unit and PostgreSQL/Trino integration coverage, container artifacts, and
  CedrusData 476-5 Kubernetes documentation.
- Keep the root `open-sdbl` library dependency-free and keep all database,
  HTTP, runtime, and Trino SPI dependencies in integration crates/modules.

## Capabilities

### New Capabilities

- `trino-catalog`: expose read-only 1C metadata and data through a Trino 476
  connector with safe pushdown.

### Modified Capabilities

- `crate-architecture`: add isolated Rust service and Java connector modules
  without changing the core dependency boundary.

## Impact

- New Rust workspace application crate and Java Maven module.
- New internal versioned HTTP protocol between Trino nodes and the Rust
  service.
- New deployment and integration-test assets.
- No write operations and no change to the SDBL language or compiler API.
