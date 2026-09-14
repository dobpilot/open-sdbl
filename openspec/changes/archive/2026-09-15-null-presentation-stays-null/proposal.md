## Why

The console printed «<unresolved reference>» for a deferred presentation
column whose value is `NULL`. A row that carries no reference has no
presentation: the platform prints nothing for it, measured on 8.3.27 by
presenting a `ВЫБОР` whose other branch is `НЕОПРЕДЕЛЕНО`. The marker is
for a reference that names no object, which is a different thing.

## What Changes

- A deferred presentation column that is `NULL` SHALL stay `NULL` in the
  console output.
- A reference that no object answers SHALL keep the unresolved marker.

## Capabilities

### Modified Capabilities

- `query-repl`: rendering a deferred presentation.

## Impact

- `crates/open-sdbl-cli/src/repl.rs`.
