## Context

The query module has grown into a large compilation unit containing public
models, six dialect-specific free-function families, parsing, metadata
projection, SQL generation, and presentation lookup compilation. PostgreSQL
and MSSQL share most semantics but differ in rendering and MSSQL date offset
configuration.

The root crate must remain dependency-free and deterministic. The refactoring
therefore needs compile-time dispatch and immutable inputs rather than runtime
drivers, connection objects, or async traits.

## Goals / Non-Goals

**Goals:**

- make `query.rs` a small public facade;
- give PostgreSQL and MSSQL separate backend values behind one generic API;
- retain one shared parser and compiler implementation;
- express provider selection with immutable values and pure methods;
- preserve generated SQL and diagnostics while intentionally replacing the
  legacy public entry points.

**Non-Goals:**

- database connections or query execution in the core crate;
- changing accepted SDBL syntax, diagnostics, or generated SQL;
- duplicating parser/compiler logic by provider;
- adding dynamic dispatch or production dependencies.

## Decisions

### Bind metadata and backend once at the compiler boundary

`QueryCompiler<'snapshot, B>` stores `&MetadataSnapshot` and an immutable
backend value. `PostgresBackend` is zero-sized; `MsSqlBackend` owns only
`year_offset`. Specialized implementations expose the same method names. This
uses static dispatch, removes database selection methods from the facade, and
introduces no mutation or hidden caches.

### Separate provider objects, share the functional core

`PostgresBackend` and `MsSqlBackend` live in separate modules. Specialized
`QueryCompiler<B>` implementations translate their immutable configuration
into the internal SQL dialect value and call the same pure parser/compiler
functions. MSSQL owns `year_offset`; PostgreSQL has no provider state.

### Remove compatibility adapters

Former functions such as `compile_postgres_query()` and
`prepare_mssql_query_with_year_offset()` are removed. Workspace callers use
`QueryCompiler<B>` directly, leaving one public compilation model instead of a
second compatibility API that would need to be maintained indefinitely.

### Keep public models provider-neutral

`CompiledQuery`, presentation models, queryable metadata models, prepared
query types, and `QueryDiagnostic` remain shared. Provider modules do not
duplicate these structures.

## Risks / Trade-offs

- The internal compiler remains substantial, but it becomes private and can be
  split further without affecting callers.
- Existing callers of legacy free functions require a source migration to the
  generic compiler; this is an intentional breaking change on the v2 branch.
- Compile-time provider objects intentionally do not own metadata or perform
  I/O, so applications still control snapshot lifetime and execution.
