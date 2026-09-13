## Why

A field of several types can be projected but not compared, which is the
largest remaining gap of the demo corpus: 40 real queries stop at
`Файлы.Редактирует В (&Список)`, `Реестр.ОбъектДанных = Файлы.Ссылка` or
`КомментарииОбъектов.ВладелецКомментария = &Владелец`. The platform's own
SQL, captured from the probe base, compares the physical members of the
field with the members the value occupies, so the rule is measurable
rather than guessed.

## What Changes

- A composite field SHALL be comparable with a value by `=`, `<>` and
  `В (…)`: the rendered predicate tests the `_TYPE` discriminator of the
  field against the value's type tag and the member that carries the
  value, exactly as the platform groups its own `В` lists.
- The value MAY be a reference (a bound parameter, `ЗНАЧЕНИЕ`, a binary
  literal), a string, a number, a date, a boolean, or a single-member
  field of those kinds; a value of a type the field cannot hold SHALL
  compare false rather than fail.
- A field whose only reference type is one table stores no `RTRef`
  member; the comparison SHALL synthesize it from the discriminator, the
  way the platform does.
- An unbound parameter SHALL render as a `NULL` comparison, which is what
  a single-member field already does.

## Capabilities

### Modified Capabilities

- `query-compilation`: comparing a composite field with a value.

## Impact

- `src/query/core/codegen/expression.rs`;
  `tests/query_types.rs`, `tests/fixtures/demo/expected.jsonl`;
  README and `docs/query-language-support.md`.
