## Why

The platform's full form lets a restriction join other tables and read
their fields in the condition: «1С:Документооборот» restricts several
catalogs with `ТекущаяТаблица ИЗ … ЛЕВОЕ СОЕДИНЕНИЕ
РегистрСведений.ДескрипторыДляОбъектов … ПО … ГДЕ …`. The library refuses
every such text, and 19 objects of the demo base for one user stay
unreadable.

## What Changes

- The expansion SHALL keep the join clauses of the full form beside the
  condition, and hand them to the compiler in the text it answers.
- The compiler SHALL compile them as a correlated predicate that keeps
  the meaning of the form: a row of the restricted table passes when the
  joined rows of that row carry at least one row satisfying the
  condition, with an outer join contributing its unmatched row.
- Restrictions of several roles SHALL be merged only when at most one of
  them joins.

## Capabilities

### Modified Capabilities

- `access-rights`: the join clauses of a restriction text.
- `query-compilation`: a restriction that joins other tables.

## Impact

`src/access.rs`, `src/query/core/parser.rs`,
`src/query/core/codegen/sources.rs`.
