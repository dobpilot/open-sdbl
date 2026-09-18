## Why

Twenty-one queries of the demo Бухгалтерия corpus write
`ИНДЕКСИРОВАТЬ ПО Псевдоним.Поле`, qualifying the index field by the
source alias, which the platform accepts; the parser refused the path.

## What Changes

- An index field of `ИНДЕКСИРОВАТЬ ПО` MAY be qualified by one or more
  leading segments; the last segment names the selection-list label the
  way an unqualified field does, and the same check applies.

## Capabilities

### Modified Capabilities

- `query-repl`: qualified index fields.

## Impact

`parse_index_fields` in `src/query/core/parser.rs`.
