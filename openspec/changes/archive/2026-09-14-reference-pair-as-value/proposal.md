## Why

A register's `Регистратор` and a document journal's `Ссылка` are stored as
an `RTRef`/`RRRef` pair without a `_TYPE` member, because such a value is
always a reference. The compiler projected the pair but refused it in
every expression — «compound field can be projected but not used in
expressions» — so `ТИПЗНАЧЕНИЯ(Журнал.Ссылка)` and an aggregate over a
recorder were reported as unsupported.

Measured on 8.3.27 with a register whose recorder has two document types:
the platform accepts the pair as a value everywhere. `ТИПЗНАЧЕНИЯ` of it
is the reference tag beside the table number the `RTRef` member carries;
`МАКСИМУМ` of it is the maximum of the concatenated pair; a comparison is
member-wise; `ЕСТЬ NULL` tests both members.

## What Changes

- A field stored as an `RTRef`/`RRRef` pair SHALL be a value of the
  runtime-typed reference kind wherever a value is expected, rendered as
  its `RTRef ‖ RRRef` payload.
- `ТИПЗНАЧЕНИЯ` of such a field SHALL answer the reference tag beside the
  table number of the row.

## Capabilities

### Modified Capabilities

- `query-compilation`: a reference pair used as a value.

## Impact

- `src/query/core/codegen/expression.rs`; `tests/query_registers.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
