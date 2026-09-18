## Why

The platform documents two more periodicities of `Обороты` and
`ОстаткиИОбороты`: `Период` — "only for the period, do not split", the
default when the argument is left out — and `Авто` — "determined by the
period fields the query reads" (`ПериодМесяц`, `ПериодГод`, …), which is
`Период` when the query reads none. UNF writes both in 67 corpus
queries; the compiler refuses them as unsupported periodicities.

## What Changes

- `Период`/`Period` as a periodicity SHALL compile exactly like an
  omitted periodicity: one row per combination of the dimensions in
  use, no `Период` column.
- `Авто`/`Auto` SHALL compile the same way, because the period fields it
  would split by (`ПериодГод`…`ПериодСекунда`) are refused already; a
  query that reads one of them keeps its diagnostic.

## Capabilities

### Modified Capabilities

- `query-repl`: two more periodicity spellings.

## Impact

`src/query/core/codegen/virtual_tables.rs`, `docs/query-language-support.md`.
