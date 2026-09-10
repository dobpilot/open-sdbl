## Why

Everyday 1C report queries are batches: aggregate into a temporary table,
add rows from another source, then join or filter it in a final statement.
The compiler rejects everything a batch needs: a second `;`-separated
statement, `ПОМЕСТИТЬ`, `ДОБАВИТЬ`, `УНИЧТОЖИТЬ`, and `ИНДЕКСИРОВАТЬ ПО`.
The console therefore cannot run 1C batch text unchanged. Because the
target databases are read-only copies, real temporary tables can never be
created; temporary tables have to be emulated inside one SELECT statement.

## What Changes

- Parse a batch of `;`-separated statements. A statement is either a query
  with an optional `ПОМЕСТИТЬ <Имя>` / `INTO` or `ДОБАВИТЬ <Имя>` / `ADD`
  clause after the selection list and an optional trailing
  `ИНДЕКСИРОВАТЬ ПО …` / `INDEX BY …` clause, or `УНИЧТОЖИТЬ <Имя>` /
  `DROP`.
- Emulate temporary tables with common table expressions: every
  `ПОМЕСТИТЬ` defines a CTE `vtN`, every `ДОБАВИТЬ` defines a new CTE
  `SELECT … FROM vtK UNION ALL <statement>` and rebinds the name,
  `УНИЧТОЖИТЬ` emits nothing and hides the name. The batch compiles to
  one statement `WITH vt… SELECT …` that contains only the CTEs reachable
  from the final statement.
- Accept `ИЗ <Имя> [КАК <Псевдоним>]` and joins on a temporary table,
  exposing its columns as fields with their column kinds (the derived-source
  rules of nested queries).
- A batch whose final statement is `ПОМЕСТИТЬ`/`ДОБАВИТЬ` returns one
  `Количество` row with the number of rows placed, as the platform does.
- Add the public `TempTablesManager` (the counterpart of 1C's
  `МенеджерВременныхТаблиц`) that keeps compiled definitions across
  compilations, plus `QueryCompiler::compile_batch`,
  `QueryCompiler::prepare_with`, and `Prepared::compile_batch`.
- Console: one session manager, `\tables` listing, `\refresh` clears it,
  temporary-table names in source completion.
- Lexer: `ДОБАВИТЬ`/`ADD`, `УНИЧТОЖИТЬ`/`DROP`, `ИНДЕКСИРОВАТЬ`/`INDEX`,
  `НАБОРАМ`/`SETS`, `УНИКАЛЬНО`/`UNIQUE`, all usable as identifiers
  outside their clause.
- New diagnostic kind `QueryDiagnosticKind::TemporaryTable`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: batches, temporary-table clauses and sources, the manager,
  console commands.
- `query-compilation`: temporary-table diagnostics.
- `crate-architecture`: the manager and batch entry points in the public
  API.
- `sdbl-lexer`: five new bilingual keywords.

## Impact

- `src/query/core/ast.rs`, `parser.rs`: `BatchAst`, `StatementAst`,
  `IntoAst`, `IndexAst`; `parse` returns a batch.
- New `src/query/core/temp_tables.rs` (public manager) and
  `src/query/core/codegen/batch.rs` (CTE assembly); `select.rs` gains a
  temporary-table source scope next to the derived one.
- `src/query.rs`: three new methods, `TempTablesManager` re-export; existing
  methods keep their signatures and accept batches.
- `crates/open-sdbl-cli`: `\tables`, session manager, completion, help.
- README, `docs/query-language-support.md`.
- Versions are bumped to 0.3.3 in a separate commit.
