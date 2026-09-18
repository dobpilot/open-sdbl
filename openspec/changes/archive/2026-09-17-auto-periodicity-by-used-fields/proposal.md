## Why

The `Авто` periodicity is documented as "determined by the period fields
the query uses": a data composition query lists `Регистратор`,
`ПериодСекунда` … `ПериодГод` in its projection and the composition
system reads whichever the report groups by. Compiling `Авто` as the
whole interval leaves every such query at "field ПериодМесяц was not
found"; 25 UNF corpus queries are of that shape.

## What Changes

- Under `Авто`, `Обороты` and `ОстаткиИОбороты` SHALL expose `Период`
  (the record period), the ten calendar levels `ПериодСекунда` …
  `ПериодГод` (`SecondPeriod` … `YearPeriod`, the beginning of that
  period of the record), `Регистратор` and `НомерСтроки`, and SHALL
  treat them as dimensions: the ones the statement reads split the
  table, the rest are summed away, so a statement that reads none gets
  the whole interval as before.
- `ОстаткиИОбороты` under `Авто` SHALL refuse its balance columns only
  when the statement also reads one of those split fields, for the same
  reason a periodic table refuses them; a period completion method SHALL
  be accepted with `Авто`.

## Capabilities

### Modified Capabilities

- `query-repl`: the `Авто` periodicity.

## Impact

`src/query/core/codegen/virtual_tables.rs`, `docs/query-language-support.md`.
