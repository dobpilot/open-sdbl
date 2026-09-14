## Why

`КАК` is optional in front of an alias. Sources already accept the short
form (`ИЗ Справочник.X Т`), projections did not, so
`КОНЕЦ ВходящийИсходящий,` — an alias written without `КАК` — was reported
as unsupported syntax and left two demo-corpus queries uncompiled.

Measured on 8.3.27: the platform accepts the short form for a field, an
aggregate, a `ВЫБОР`, a constant and a dereference, names the column by
it in `УПОРЯДОЧИТЬ ПО`, `СГРУППИРОВАТЬ ПО` and `ИТОГИ`, and takes exactly
one word (`Имя Второе` is a syntax error). A contextual keyword may be the
alias (`ВЫБРАТЬ 1 Сумма` names a column `Сумма`), a reserved one may not
(`ВЫБРАТЬ 1 Выбор` is a syntax error).

## What Changes

- A projection SHALL accept an alias written without `КАК`, naming the
  column exactly as the `КАК` form does.
- A word that opens the next clause — `ИТОГИ`, `ИНДЕКСИРОВАТЬ` — SHALL
  never be read as such an alias, in a projection or after a source.
- A wildcard projection SHALL keep refusing an alias in either form.

## Capabilities

### Modified Capabilities

- `query-compilation`: an alias written without `КАК`.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
