## Why

The compound fields of the accounting relations — `Субконто<k>`,
`КорСубконто<k>`, the recorder — labelled every column by the bare
query name, so a `UNION` that puts `НЕОПРЕДЕЛЕНО` or a plain reference
against such a field could not tell its members apart and refused the
union as a width mismatch (a demo Бухгалтерия query does exactly that).

## What Changes

- A compound field exposed by an accounting virtual table or the record
  tables SHALL label its columns with the member suffix — `Имя_TYPE`,
  `Имя_S`, …, `Имя` for the reference member — the way the metadata
  fields do, so union spreading and aliasing read them.

## Capabilities

### Modified Capabilities

- `query-repl`: labels of compound accounting fields.

## Impact

`relabel` in `src/query/core/codegen/accounting.rs`; recorded labels of
unaliased compound projections change from `Имя`/`Имя_2` to
`Имя_TYPE`/`Имя`.
