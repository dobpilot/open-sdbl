## 1. Syntax

- [x] 1.1 Add `ДОБАВИТЬ`/`ADD`, `УНИЧТОЖИТЬ`/`DROP`, `ИНДЕКСИРОВАТЬ`/`INDEX`,
  `НАБОРАМ`/`SETS`, `УНИКАЛЬНО`/`UNIQUE` to the lexer table (56 entries)
  and to the parser's contextual identifiers.
- [x] 1.2 Parse batches: `BatchAst`/`StatementAst`, `ПОМЕСТИТЬ`/`ДОБАВИТЬ`
  after the first selection list, trailing `ИНДЕКСИРОВАТЬ ПО` in both
  forms with `УНИКАЛЬНО`, `УНИЧТОЖИТЬ`, the 64-statement bound, and a
  bare-identifier source `ИЗ Имя [КАК Псевдоним]`.

## 2. Manager and SQL generation

- [x] 2.1 Add `TempTablesManager`/`TempTable` in
  `src/query/core/temp_tables.rs` with dialect and snapshot binding, the
  256-definition bound, `tables`, `contains`, `is_empty`, `clear`, and the
  `QueryDiagnosticKind::TemporaryTable` variant.
- [x] 2.2 Compile definitions as CTE bodies with the nested-query rules,
  validate `ИНДЕКСИРОВАТЬ ПО` fields against output labels, implement
  `ДОБАВИТЬ` with the strict positional structure check, `УНИЧТОЖИТЬ`
  hiding, name reuse after drop, and duplicate/unknown-name diagnostics.
- [x] 2.3 Build temporary-table source scopes from stored columns for
  `ИЗ`, joins, nested queries, and `IN` subqueries; track CTE dependencies;
  assemble `WITH` with the reachable closure in id order; emit the
  `Количество` statements for final `ПОМЕСТИТЬ`/`ДОБАВИТЬ`.
- [x] 2.4 Add `QueryCompiler::compile_batch`, `QueryCompiler::prepare_with`,
  `Prepared::compile_batch`; route `compile`/`compile_with`/`prepare`
  through a throwaway manager with the no-rows diagnostic; commit manager
  state only on success; re-export `TempTablesManager` and `TempTable`.

## 3. Console

- [x] 3.1 Keep one session manager, run statements through `prepare_with`
  and `Prepared::compile_batch`, report dropped tables for `None`, clear the
  manager on `\refresh` with a notice.
- [x] 3.2 Add `\tables` (help text, command table, completion candidates)
  and temporary-table names to source completion.

## 4. Verification and documentation

- [x] 4.1 Core goldens on both dialects: `ПОМЕСТИТЬ` then read; join to a
  temporary table; `ДОБАВИТЬ` chain; drop and redefine; batch ending in
  `ПОМЕСТИТЬ` and in `ДОБАВИТЬ` (`Количество`); unreachable CTE omitted;
  temporary table inside a nested source and an `IN` subquery; dereference
  through a temporary-table reference column with a deferred presentation
  request; parameter inlined into a definition and reused from the manager
  without values; MSSQL date read from a temporary table offset once;
  `ИНДЕКСИРОВАТЬ ПО НАБОРАМ … УНИКАЛЬНО` accepted and ignored.
- [x] 4.2 Diagnostics tests: unknown, hidden, duplicate name; `ДОБАВИТЬ` to
  a missing table and structure mismatch; `УНИЧТОЖИТЬ` unknown; `*`,
  deferred presentation, and ordering without `ПЕРВЫЕ` inside a definition;
  `ПОМЕСТИТЬ` inside a nested query; index field outside the selection
  list; batch ending with `УНИЧТОЖИТЬ` through `compile`; manager dialect
  and snapshot mismatch; statement and definition bounds; failed batch
  leaves the manager unchanged.
- [x] 4.3 CLI tests: `\tables` in help and completion, listing after a
  definition, drop notice, `\refresh` clearing.
- [x] 4.4 Update README (console command table, API section with a
  `TempTablesManager` example, frozen-parameter note) and
  `docs/query-language-support.md` rows for `ПОМЕСТИТЬ`/`УНИЧТОЖИТЬ`,
  `ДОБАВИТЬ`, `ИНДЕКСИРОВАТЬ ПО`, and batches.
- [x] 4.5 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
