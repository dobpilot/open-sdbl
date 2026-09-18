## Why

The conditions of the aggregating virtual tables accepted direct fields
only. Configurations filter through a reference — `Счет.ТипСчета =
ЗНАЧЕНИЕ(…)` in every UNF accounting report, `Номенклатура.Родитель =
&Р` in stock reports — and the platform accepts it; 15 accounting and 11
accumulation queries of the UNF corpus stop on "condition supports
direct dimensions and separators only".

## What Changes

- A dereference in the condition or the account condition of `Остатки`,
  `Обороты` and `ОстаткиИОбороты` of either register kind SHALL join the
  target table to the relation the condition filters — each branch of a
  folded accounting table on that branch's own account, the totals and
  the movement branch of an accumulation balance on their own alias —
  with the same `LEFT JOIN` and type guard an ordinary query renders.
- The access restriction's own dereferences SHALL be rendered the same
  way instead of being refused.

## Capabilities

### Modified Capabilities

- `query-repl`: dereferences in virtual-table conditions.

## Impact

`src/query/core/codegen/virtual_tables.rs` (the condition compiler
returns its joins), `accounting.rs`; `docs/query-language-support.md`.
The slice tables keep refusing a dereference for now.
