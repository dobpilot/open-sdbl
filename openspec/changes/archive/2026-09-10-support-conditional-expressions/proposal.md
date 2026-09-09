## Why

Reports written in the 1C query language lean on three expression forms the
compiler still rejects: `ВЫБОР … КОНЕЦ` for conditional values,
`ЕСТЬNULL(x, y)` for default values after outer joins, and `ПОДОБНО` for
pattern filters. Aggregates accept only a bare field, so the common
`СУММА(ВЫБОР КОГДА … ТОГДА Сумма ИНАЧЕ 0 КОНЕЦ)` shape cannot be written
even once `ВЫБОР` exists. All four are pure expression features with a direct
SQL rendering on both providers.

## What Changes

- Parse `ВЫБОР КОГДА <predicate> ТОГДА <value> … [ИНАЧЕ <value>] КОНЕЦ` /
  `CASE WHEN … THEN … [ELSE …] END` (searched form only, as in 1C) and render
  it as SQL `CASE`; the column kind is the first non-wildcard branch kind and
  every branch kind must be compatible.
- Recognize `ЕСТЬNULL`/`ISNULL` as a bilingual keyword and compile
  `ЕСТЬNULL(x, y)` to `COALESCE(x, y)` with the same kind rule.
- Recognize `ПОДОБНО`/`LIKE` and `СПЕЦСИМВОЛ`/`ESCAPE`; compile
  `<value> [НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ <escape>]` to
  `[NOT] (… LIKE … [ESCAPE …])` on both providers, passing the pattern through
  unchanged.
- Accept any scalar expression as the argument of `СУММА`, `МИНИМУМ`,
  `МАКСИМУМ`, and `КОЛИЧЕСТВО([РАЗЛИЧНЫЕ] …)`.
- Widen reference operands of `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ` to one
  runtime-typed payload when their targets or widths differ, instead of
  emitting columns of mixed width.
- Treat every expression whose kind is boolean as a predicate on MSSQL and
  every expression whose kind is date-time as a date expression for the
  year-offset correction, replacing the syntactic checks.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: three more bilingual keywords (`ЕСТЬNULL`, `ПОДОБНО`,
  `СПЕЦСИМВОЛ`).
- `query-repl`: conditional, default-value, and pattern expressions; aggregates
  over expressions; kind-driven predicate and date handling.
- `query-compilation`: reference widening across UNION branches.

## Impact

- `Expression` gains `Case`, `IsNullFunction`, and `Like` variants;
  `AggregateArgument::Field` becomes `AggregateArgument::Expression`.
- Lexer table grows from 48 to 51 entries.
- Existing goldens are unchanged: the new rendering paths are reached only by
  the new syntax.
- `docs/query-language-support.md` and README gain the new rows and examples.
