## Why

`ВидДвижения` is how an accumulation register's movement type is written in
a query; the compiler accepted only the SchemaStorage spelling
`RecordKind`, so a demo-corpus query reading it reported an unknown field.

Measured on 8.3.27: `ВЫБРАТЬ Р.ВидДвижения ИЗ РегистрНакопления.X КАК Р`
answers the movement type of every record.

## What Changes

- `ВидДвижения` / `RecordType` SHALL name the `RecordKind` standard field,
  next to its SchemaStorage spelling.

## Capabilities

### Modified Capabilities

- `query-compilation`: the name of the movement-type standard field.

## Impact

- `src/query/core/resolve.rs`; `tests/query_registers.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
