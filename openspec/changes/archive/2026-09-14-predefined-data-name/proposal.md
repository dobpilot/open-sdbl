## Why

`ИмяПредопределенныхДанных` is the largest remaining gap the corpus of
real demo queries shows: 94 of its 397 queries name it and 49 are blocked
by nothing else. The platform does not store the name. It stores the
predefined GUID in `PredefinedID` and maps it back to the symbolic name
declared in the configuration, which the resolver already decodes from
the `<object>.1c` Config resource.

## What Changes

- `ИмяПредопределенныхДанных` / `PredefinedDataName` SHALL resolve to the
  symbolic name of the predefined item the row is, and to an empty string
  for every other row, on any object that stores `PredefinedID`.
- It SHALL answer wherever a stored field does, including through a
  reference, where a missing outer-join row keeps `NULL`.
- The demo metadata fixture SHALL carry the predefined-value resources of
  the objects it already describes, so the corpus test sees the names.

## Capabilities

### Modified Capabilities

- `query-repl`: the predefined-data name field.

## Impact

- `src/query/core/codegen/context.rs`;
  `tests/fixtures/demo/config.pack` and `expected.jsonl`;
  README and `docs/query-language-support.md`.
