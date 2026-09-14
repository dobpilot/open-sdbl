## Why

Two constructs the platform accepts and the compiler refused, both
measured on the probe base. A projection that reads no field — a label
such as `ВЫРАЗИТЬ("Все" КАК СТРОКА(20))` — needs no grouping key and
stands beside an aggregate. And `+` concatenates strings: `Наименование +
" ("` answers «Вантус (», while mixing a string with a number is refused
as «Неверные параметры "+"».

## What Changes

- A projection that reads no field SHALL be accepted in a grouped
  statement without being a grouping key, and beside an aggregate without
  `СГРУППИРОВАТЬ ПО`.
- `+` over strings SHALL compile as concatenation, cast to text on both
  providers; an operand of another type beside a string SHALL be refused,
  as the platform refuses it.

## Capabilities

### Modified Capabilities

- `query-compilation`: constants of the row set and string concatenation.

## Impact

- `src/query/core/codegen/sources.rs`, `select.rs`, `expression.rs`,
  `dialect.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
