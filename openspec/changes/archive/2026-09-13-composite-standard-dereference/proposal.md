## Why

`Т.Регистратор.Дата` and `Т.Объект.Родитель` are ordinary 1C, and the
compiler answered «field … was not found in any target». Measured cause:
SchemaStorage does not name the targets of a reference that admits
several tables — it writes `["R", 0, 0, "", 4]` where a single-target
column writes `["R", 0, 0, "Reference53", 3]`. The dereference therefore
falls back to scanning the snapshot for an *attribute* of that name, and
standard fields are not attributes, so none was ever found.

## What Changes

- The scan SHALL treat a standard-field name as belonging to every
  reference object, in all its accepted spellings, so a standard field
  reached through such a reference resolves. The candidate limit and its
  `ВЫРАЗИТЬ` advice stay as they are.
- The standard fields the scan knows SHALL include `ParentID`,
  `OwnerID`, `Folder`, `Predefined` and `RecordKind` beside the twelve
  it already knew.

## Capabilities

### Modified Capabilities

- `query-repl`: dereferencing a standard field through a composite
  reference.

## Impact

- `src/query/core/resolve.rs`, `codegen/context.rs`;
  `docs/query-language-support.md`.
