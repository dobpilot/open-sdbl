## Why

`ОстаткиИОбороты` is the table every stock or settlement report reads: it
answers the opening balance, the receipts and expenses of an interval,
their turnover, and the closing balance in one pass. The compiler did not
know it.

## What Changes

- The lexer SHALL recognize `ОСТАТКИИОБОРОТЫ`/`BALANCEANDTURNOVERS` as a
  keyword that stays a contextual identifier.
- `РегистрНакопления.X.ОстаткиИОбороты(Начало, Конец, Периодичность,
  МетодДополненияПериодов, Условие)` SHALL compile for a balance
  register, exposing `<Ресурс>НачальныйОстаток`, `<Ресурс>Приход`,
  `<Ресурс>Расход`, `<Ресурс>Оборот` and `<Ресурс>КонечныйОстаток` beside
  the dimensions, and SHALL sum them over the dimensions the statement
  never reads, like the other register tables.
- The values SHALL be read from the movements: the opening balance is the
  signed movement before `Начало`, the receipts and expenses are the
  movements of `[Начало, Конец)` split by record kind, and the closing
  balance is their sum.
- The periodicity and the period completion method SHALL be
  `UnsupportedFeature` diagnostics for now: per-period rows need running
  balances, which SQL Server 2008 cannot express with a window function.

## Capabilities

### Modified Capabilities

- `sdbl-lexer`: one new bilingual keyword.
- `query-repl`: the `ОстаткиИОбороты` virtual table.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/virtual_tables.rs`; README and
  `docs/query-language-support.md`.
