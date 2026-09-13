## Why

1C queries walk reference chains freely:
`Т.Контрагент.ОсновнойДоговор.Валюта.Наименование`. The compiler stopped
at one hop and answered «reference paths deeper than one hop are not
supported», which is the limitation a real query hits first.

## What Changes

- A reference path SHALL walk any number of hops: every hop but the last
  joins its target to the alias the previous hop produced, and the last
  segment is read from the table the walk ended on.
- Identical hops SHALL reuse one join, as one-hop dereferences already
  do, so repeating a prefix costs nothing.
- The walk SHALL continue only through references to a single table. A
  composite reference selects its value by type and has no single table
  to continue from, so a deeper path through one SHALL be an
  `UnsupportedFeature` diagnostic naming the field.

## Capabilities

### Modified Capabilities

- `query-repl`: reference paths of any depth.

## Impact

- `src/query/core/codegen/context.rs`; README and
  `docs/query-language-support.md`.
