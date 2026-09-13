## Why

The 1C source list accepts several elements separated by commas
(`ИЗ Справочник.Товары КАК А, Справочник.Клиенты КАК Б ГДЕ …`), each
element being a table or a join chain. The compiler accepts one element
only, so such queries fail at the parser even though the Cartesian
product is plain SQL on both providers.

## What Changes

- The parser SHALL accept a comma-separated source list where every
  element is a table, nested query, temporary table, virtual table, or
  the constants table, optionally followed by its own joins.
- The compiler SHALL render the list as a left-to-right `CROSS JOIN`
  chain, keeping the written order, so that the existing join machinery
  (dereference joins, separator placement, ordering rules) applies
  unchanged: a comma-listed source is filtered like the base source.
- A join condition SHALL see only the sources of its own comma element,
  as the platform does (`Поле не найдено` on the platform, `UnknownField`
  here).
- `ПОЛНОЕ СОЕДИНЕНИЕ` and `*` keep their single-join restrictions: with
  several sources they stay `UnsupportedFeature` diagnostics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: the source list grammar and its rendering.

## Impact

- `src/query/core/ast.rs` (`JoinKind::Cross`, optional join condition),
  `parser.rs`, `codegen/select.rs`; README and
  `docs/query-language-support.md`.
