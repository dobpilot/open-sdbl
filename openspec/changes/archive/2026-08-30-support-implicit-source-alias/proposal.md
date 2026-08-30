## Why

The query compiler accepts source aliases only after `КАК`/`AS`, while the
1C query language also permits an alias directly after the metadata source.
As a result, valid paths such as `t.Регистратор.Номер` fail before
metadata resolution.

## What Changes

- Accept a source alias both with and without `КАК`/`AS`.
- Keep clause keywords from being consumed as implicit aliases.
- Resolve alias-qualified fields and one-hop reference properties identically
  for explicit and implicit aliases.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: clarify and implement both valid source-alias spellings.

## Impact

- Dependency-free query parsing and its conformance tests only.
- No database acquisition or CLI connection behavior changes.
