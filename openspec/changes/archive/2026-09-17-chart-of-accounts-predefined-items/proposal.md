## Why

A chart of accounts keeps its predefined accounts not in a `<guid>.1c`
resource like a catalog but in `<guid>.9` — measured on the UNF base:
the chart `Управленческий` has 61 predefined accounts, no `.1c`, and a
`.9` value table whose rows carry the account's reference, name, code
and description. The corpus names them in
`ЗНАЧЕНИЕ(ПланСчетов.Управленческий.ПрочиеРасходы)` and stops on
"VALUE currently supports only catalogs and enumerations".

## What Changes

- The application adapters SHALL also load `<guid>.9` resources, and
  the core SHALL decode their predefined rows the way it decodes `.1c`
  rows: a row is `{2, <index>, <column count>, {"#", <type>, {1,
  <guid>}}, …}` and the name is its first string column. A decoded
  value SHALL remember which resource kind it came from, and resolution
  SHALL keep `.9` values for charts of accounts only and `.1c` values
  for the other kinds only, so a `.9` resource of another class never
  becomes a predefined value.
- `ЗНАЧЕНИЕ(ПланСчетов.<Имя>.<Счет>)` SHALL resolve through
  `_PredefinedID` as a catalog value does. Charts of characteristic types
  and of calculation types keep their items elsewhere (`.7`, `.3` by the
  UNF survey, shared with other classes) and stay refused until measured
  on the probe base.
- `tools/corpus/fetch_base.py` SHALL pack `.9` resources.

## Capabilities

### Modified Capabilities

- `onec-metadata`: `.9` predefined resources of charts of accounts.
- `query-repl`: `ЗНАЧЕНИЕ` over a chart of accounts.

## Impact

`src/metadata/config.rs`, `queries.rs`, `resolve.rs`;
`src/query/core/codegen/sources.rs`; `tests/support/mod.rs`;
`tools/corpus/fetch_base.py`; `ConfigPredefinedValue` gains a public
`source` field.
