## Why

Joined queries can project a one-hop reference property, but applying
`ПРЕДСТАВЛЕНИЕССЫЛКИ` to that property fails after the dereference join has
already been planned. Real tabular-section queries need chained presentation
lookups such as `Ссылка.ДоговорКонтрагента` and
`ЦФО.Сам_БизнесРегион`.

## What Changes

- Allow presentation functions to consume a supported one-hop dereferenced
  field in a joined projection.
- Track the exact source alias and owner object for every resolved joined path.
- Build the presentation join from the dereference alias instead of the
  original metadata source.
- Reuse compatible dereference and presentation joins deterministically.
- Return universal references as typed deferred-presentation payloads and let
  the application resolve only the references present in the bounded result.
- Provide safe core-generated batch lookup SQL for deferred presentations.
- Preserve PostgreSQL, MSSQL, and transposed FULL JOIN behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: present one-hop dereferenced reference fields through joins.

## Impact

- Extends `CompiledQuery` with deferred-presentation column metadata and adds
  core helpers for safe batch lookup SQL.
- Generated queries may contain a bounded chain of two auxiliary LEFT JOINs:
  one for the selected property and one for that property's presentation.
- The CLI can execute additional bounded lookup queries after the main query
  when a field is stored as a universal reference with a runtime `_RTRef`.
