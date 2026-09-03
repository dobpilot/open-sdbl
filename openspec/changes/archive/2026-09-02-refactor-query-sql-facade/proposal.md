## Why

`src/query.rs` currently mixes the public query API, parser, metadata
resolution, shared compilation pipeline, and both SQL dialect entry points in
one module. Applications also have to select among several similarly named
free functions. This makes dialect-specific evolution difficult and obscures
the dependency-free functional core.

## What Changes

- Introduce generic `QueryCompiler<B>` as the public entry point bound to one
  immutable `MetadataSnapshot` and backend value.
- Expose separate immutable `PostgresBackend` and `MsSqlBackend` values. The
  specialized `QueryCompiler` implementations provide uniform compile,
  prepare, presentation-plan, and deferred presentation lookup operations.
- Move the parser and shared compiler implementation out of the facade module.
- Remove the former database-named free functions and migrate workspace callers
  to the generic compiler API.
- Preserve generated SQL, diagnostics, presentation requests, and result
  metadata.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `crate-architecture`: expose a cohesive database-specific query facade while
  retaining the dependency-free core boundary.

## Impact

- Adds a zero-cost generic compiler and removes legacy free-function entry
  points as an intentional v2 API cleanup.
- Changes source-module organization only; no database I/O or dependencies are
  added to the root crate.
- External callers using legacy free functions must migrate to
  `QueryCompiler<B>`.
