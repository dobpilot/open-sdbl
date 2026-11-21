## Why

Every accounting report reads the virtual tables of `РегистрБухгалтерии`:
balances and turnovers by account and extra dimensions, debit/credit
turnovers, and the records with their extra dimensions. The compiler knows
the main table of an accounting register only as a plain table, without
the debit/credit split of its fields, the extra-dimension table, the chart
of accounts behind it, or any of the six virtual tables. The UNF corpus
(`tests/fixtures/unf`) shows which of them the configuration actually
reads, and the platform probe base shows how the platform answers them.

## What Changes

- The metadata decoder SHALL recognize the accounting-register Config
  collections (dimensions with their balance flag, resources with their
  balance flag, attributes), the register's chart of accounts and
  correspondence setting, and the chart of accounts' extra-dimension count
  and per-account extra-dimension kinds.
- The main table SHALL expose `СчетДт`/`СчетКт` (or `Счет` without
  correspondence), `<Измерение>Дт`/`<Измерение>Кт` and
  `<Ресурс>Дт`/`<Ресурс>Кт` for non-balance fields, and the
  `Субконто` service table SHALL be queryable with `Вид`, `ВидДвижения`,
  `Значение` beside the record key.
- The lexer SHALL recognize `ОБОРОТЫДТКТ`/`DRCRTURNOVERS`,
  `ДВИЖЕНИЯССУБКОНТО`/`RECORDSWITHEXTDIMENSIONS` and
  `СУБКОНТО`/`EXTDIMENSIONS` as contextual keywords.
- `РегистрБухгалтерии.X.ДвиженияССубконто`, `.Обороты`, `.ОборотыДтКт`,
  `.Остатки` and `.ОстаткиИОбороты` SHALL compile, in stages, with the
  account condition, the extra-dimension list, the condition, and the
  period arguments the platform accepts; what a stage refuses SHALL be an
  `UnsupportedFeature` diagnostic that names the argument.

## Capabilities

### Modified Capabilities

- `onec-metadata`: accounting-register field purposes and chart-of-accounts
  extra-dimension metadata.
- `sdbl-lexer`: three new bilingual contextual keywords.
- `query-repl`: the accounting-register main-table fields, the extra
  dimension table, and the five aggregating virtual tables.

## Impact

- `src/metadata/config.rs` (collection GUIDs, flags), `resolve.rs`
  (chart-of-accounts links), `src/lexer.rs`, `src/query/core/ast.rs`,
  `parser.rs`, `resolve.rs`, `codegen/virtual_tables.rs` (a new
  `accounting.rs` next to it), `codegen/sources.rs`.
- `tests/fixtures/unf` (new corpus), `tests/query_accounting.rs` (new),
  README and `docs/query-language-support.md`.
- No new production dependency.
