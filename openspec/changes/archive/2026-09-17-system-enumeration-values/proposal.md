## Why

Configurations compare the movement type of a register record with
`ЗНАЧЕНИЕ(ВидДвиженияНакопления.Приход)`, the side of an extra dimension
with `ЗНАЧЕНИЕ(ВидДвиженияБухгалтерии.Дебет)`, and the kind of an
account with `ЗНАЧЕНИЕ(ВидСчета.АктивноПассивный)`. The compiler knows
`ЗНАЧЕНИЕ` only for a three-segment metadata path and refuses the
two-segment system enumeration; about sixty queries of the UNF corpus
stop there.

## What Changes

- `ЗНАЧЕНИЕ` SHALL accept the three system enumerations by their Russian
  and English names and compile a value to the number the platform
  stores: `ВидДвиженияНакопления` (`Приход` 0, `Расход` 1),
  `ВидДвиженияБухгалтерии` (`Дебет` 0, `Кредит` 1), `ВидСчета`
  (`Активный` 0, `Пассивный` 1, `АктивноПассивный` 2). The account kinds
  are measured on the UNF chart of accounts (`_Kind` against the
  predefined accounts); the record kind is the `_RecordKind` column the
  balance tables already switch on.
- The chart of accounts SHALL expose its standard fields `Вид`,
  `Забалансовый` and `Порядок`, so `Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.…)`
  resolves.

## Capabilities

### Modified Capabilities

- `query-repl`: system enumeration values and chart-of-accounts standard
  fields.

## Impact

- `src/query/core/ast.rs`, `parser.rs`, `resolve.rs`,
  `codegen/expression.rs`, `codegen/sources.rs`, `codegen/select.rs`;
  `docs/query-language-support.md`.
- The value is a number, so `ТИПЗНАЧЕНИЯ` of it answers the number type
  where the platform answers the enumeration type; documented.
