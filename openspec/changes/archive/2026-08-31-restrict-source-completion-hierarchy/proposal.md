## Why

The console currently filters one global completion catalog regardless of
cursor context. Immediately after `ИЗ`/`FROM`, Tab therefore offers field aliases,
bare object names, and physical PostgreSQL tables alongside valid metadata
sources. A query source should instead follow the explicit
`Type.MetadataName` hierarchy.

## What Changes

- Build a dedicated completion catalog for query sources.
- After `ИЗ`/`FROM` and `СОЕДИНЕНИЕ`/`JOIN`, offer only Russian- or
  English-kind-qualified metadata objects and their valid virtual tables.
- Keep commands, keywords, fields, reference paths, and physical diagnostics
  available in non-source completion contexts.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: make metadata completion aware of query-source context.

## Impact

- Observable interactive completion behavior only.
- No parser, compiler, metadata API, or generated SQL change.
