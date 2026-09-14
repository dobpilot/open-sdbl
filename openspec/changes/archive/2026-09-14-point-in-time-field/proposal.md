## Why

`МоментВремени` is the standard field that orders a document, or a
register record, past the second the date alone resolves: it is the pair
of the date and the reference behind the row. The compiler reported it as
an unknown field, which left two demo-corpus queries uncompiled.

Measured on 8.3.27: for a document the platform reads `_Date_Time` and
`_IDRRef`, for a register record `_Period` and `_Recorder…`; it spreads
the pair over two SQL columns in the projection, over two terms in
`УПОРЯДОЧИТЬ ПО`, `СГРУППИРОВАТЬ ПО` and `РАЗЛИЧНЫЕ`, and compares two
points in time lexicographically. It refuses the field on a catalog and
on an independent information register, refuses `МАКСИМУМ` of it and
refuses dereferencing it.

## What Changes

- `МоментВремени` / `PointInTime` SHALL resolve on a document and on a
  register record subordinate to a recorder, as the pair of that row's
  date and reference.
- The pair SHALL be projected as two columns, the date labelled
  `<имя>_T` and the reference `<имя>`, and SHALL expand the same way in
  ordering, grouping and `РАЗЛИЧНЫЕ`.
- Every other source SHALL keep reporting the field as unknown, as the
  platform does.

## Capabilities

### Modified Capabilities

- `query-compilation`: the point-in-time standard field.

## Impact

- `src/query/core/codegen/context.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
