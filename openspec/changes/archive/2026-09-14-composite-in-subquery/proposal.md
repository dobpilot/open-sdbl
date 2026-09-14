## Why

A subquery whose value is composite projects one column per member, and
`В (…)` demanded exactly one column, so `Ссылка В (ВЫБРАТЬ
П.СоставноеПоле …)` was refused.

Measured on 8.3.27: the platform compares the members side by side —
`(T1._Fld70_TYPE, T1._Fld70_S, T1._Fld70_RTRef, T1._Fld70_RRRef) IN
(SELECT …)` — spreading the other side over the same members: its own
member carries the value, the discriminator its tag, every other member
the zero of its type, and each of them stays `NULL` while the value is.

## What Changes

- `В (<подзапрос>)` SHALL accept a subquery whose columns are the members
  of one composite value, comparing the members side by side.
- A subquery whose columns are separate values SHALL keep its diagnostic.
- SQL Server has no row comparison, so the composite form SHALL be refused
  there with a diagnostic naming the reason.

## Capabilities

### Modified Capabilities

- `query-compilation`: a composite subquery of `В (…)`.

## Impact

- `src/query/core/codegen/expression.rs`; `tests/query_refs.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
