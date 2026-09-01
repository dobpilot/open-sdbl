## 1. Contract and architecture

- [x] 1.1 Inspect the workspace, metadata acquisition, `MetadataSnapshot`,
  queryable fields, and Trino 476 Thrift/SPI sources.
- [x] 1.2 Document why stock Thrift cannot satisfy limit pushdown and select
  the Java SPI fallback.
- [x] 1.3 Add architecture and split documentation and validate this change
  strictly.

## 2. Rust service vertical slice

- [x] 2.1 Add isolated configuration, PostgreSQL pool, metadata loader, and
  immutable TTL cache.
- [x] 2.2 Add deterministic schema/table/column and explicit type mapping.
- [x] 2.3 Add versioned metadata and streaming scan HTTP endpoints.
- [x] 2.4 Translate typed domains to quoted, parameterized PostgreSQL SQL and
  apply projection and limit remotely.
- [x] 2.5 Add health, readiness, metrics, structured logging/errors, timeouts,
  and row safeguards.

## 3. Trino 476 plugin

- [x] 3.1 Add a minimal read-only Maven plugin pinned to `trino-spi:476`.
- [x] 3.2 Implement metadata, handles, one real split, and streaming records.
- [x] 3.3 Implement safe filter, projection, and guaranteed limit pushdown.
- [x] 3.4 Add SPI/unit tests and plugin packaging.

## 4. Verification and deployment

- [x] 4.1 Add Rust unit tests for names, conflicts, types, quoting, domains,
  SQL parameters, and cache behavior.
- [x] 4.2 Add PostgreSQL integration tests and a Trino 476 compose environment
  covering discovery, describe, scan, filter, projection, and limit.
- [x] 4.3 Add Dockerfiles, example catalog/configuration, Kubernetes manifests,
  and CedrusData 476-5 documentation.
- [x] 4.4 Add README end-to-end usage and query examples.
- [x] 4.5 Run formatting, warnings-denied Clippy, tests, rustdoc, Java tests,
  integration smoke tests where available, and strict OpenSpec validation.
