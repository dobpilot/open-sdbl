## Why

Stage 1 of the accounting-register plan
(`accounting-register-virtual-tables`): before any virtual table can be
compiled, the compiler has to know an accounting register's fields the
way it knows an accumulation register's. Today the register's
dimensions, resources and attributes carry no purpose, a non-balance
field is exposed as `Fld414Dt`, the accounts as `AccountDt`, the chart
of accounts is a plain reference target, and every accounting virtual
table stops on a wrong argument count.

## What Changes

- The decoder SHALL recognize the accounting-register dimension,
  resource and attribute collections and the balance flag of a dimension
  or resource, and SHALL expose the chart of accounts a register is bound
  to; all measured on the UNF register `Управленческий` on 8.3.27.
- The main table SHALL expose `СчетДт`/`СчетКт` (`Счет` without
  correspondence), balance fields by name, and non-balance fields as
  `<Имя>Дт`/`<Имя>Кт` (`<Name>Dr`/`<Name>Cr`), with a physical
  `Fld<N>Dt`/`Fld<N>Ct` column attributed to field `N`.
- The lexer SHALL recognize `ОБОРОТЫДТКТ`/`DRCRTURNOVERS` and
  `ДВИЖЕНИЯССУБКОНТО`/`RECORDSWITHEXTDIMENSIONS`; the parser SHALL take
  the platform's argument counts for the accounting tables (4, 8, 7, 8,
  5), and every accounting virtual table SHALL be an `UnsupportedFeature`
  diagnostic naming the table until its stage lands.

## Capabilities

### Modified Capabilities

- `onec-metadata`: accounting-register field purposes, balance flag,
  chart-of-accounts link.
- `sdbl-lexer`: two keywords.
- `query-repl`: main-table fields and the arity of accounting virtual
  tables.

## Impact

`src/metadata/config.rs`, `resolve.rs`; `src/lexer.rs`;
`src/query/core/ast.rs`, `parser.rs`, `resolve.rs`,
`codegen/virtual_tables.rs`; `ConfigDescriptor`, `MetadataField` and
`MetadataObject` gain public fields (`balance`, `chart_of_accounts`).
