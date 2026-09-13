## Why

`ИТОГИ … ПО …` is how 1C reports get subtotal rows, and every report
query built by the query wizard ends with it. The compiler rejects the
clause, so such queries have to be stripped by hand before they can be
checked against the database.

## What Changes

- The lexer SHALL recognize `ИТОГИ`/`TOTALS`, `ОБЩИЕ`/`OVERALL`,
  `ИЕРАРХИЯ`/`HIERARCHY`, `ТОЛЬКО`/`ONLY`, and `ПЕРИОДАМИ`/`PERIODS` as
  contextual keywords.
- The parser SHALL accept `ИТОГИ [<итоговое поле> [КАК Псевдоним], …]
  ПО [ОБЩИЕ] [<контрольная точка> [[ТОЛЬКО] ИЕРАРХИЯ | ПЕРИОДАМИ(…)]
  [КАК Псевдоним], …]` at the end of a top-level statement.
- The compiler SHALL render the result the way the platform's linear
  traversal (`ОбходРезультатаЗапроса.Прямой`) presents it: the overall
  row, then for every control-point value its total row followed by its
  detail rows or nested totals, groups ordered by first appearance in
  the ordered detail result. Total rows carry the control points of
  their level and above, the totals fields aggregated over the result
  columns, and `NULL` elsewhere.
- `ПЕРИОДАМИ` SHALL be parsed and validated but SHALL NOT change the
  rows, because the platform adds empty periods only to the tree
  traversal with period completion, not to the linear result.
- `ИЕРАРХИЯ`/`ТОЛЬКО ИЕРАРХИЯ` SHALL be parsed and refused as
  `UnsupportedFeature` until `hierarchy-totals`.
- `CompileOptions::totals_level(true)` SHALL append a numeric `__level`
  column reporting the platform's `Уровень()`: `0` for the overall row,
  `1…n` for control-point totals, `n + 1` (or `n` without `ОБЩИЕ`) for
  detail rows. The console SHALL enable it.
- `ИТОГИ` with `ПОМЕСТИТЬ`/`ДОБАВИТЬ` SHALL be a `Syntax` diagnostic and
  `ИТОГИ` inside a nested query an `UnsupportedFeature` diagnostic.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: five new bilingual keywords.
- `query-repl`: totals rows, ordering, the level column option, and the
  console switch.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/totals.rs` (new), `codegen/select.rs` (hidden order
  columns), `codegen/orchestrate.rs`, `codegen/batch.rs` (`WITH`
  merging), `params.rs`; CLI console; README and
  `docs/query-language-support.md`.
- Window functions (`ROW_NUMBER`, `MIN … OVER`) are used, available on
  PostgreSQL 8.4+ and SQL Server 2005+, within the portability targets.
