## Why

The corpus of real demo queries shows the two names that block it most:
`Предопределенный` in 88 queries and `ЭтоГруппа` in 28, together a third
of everything the compiler refused. Neither is a stored column: the
platform computes them, which is why they had no alias.

## What Changes

- `ЭтоГруппа`/`IsFolder` SHALL resolve to the negation of the stored
  `Folder` column, which holds true for an item and false for a folder,
  and `Предопределенный` SHALL resolve to the `PredefinedID` column
  differing from the empty reference. Both are measured on the platform.
- They SHALL answer wherever a stored field does: projection, `ГДЕ`,
  `СГРУППИРОВАТЬ ПО`, `УПОРЯДОЧИТЬ ПО`, and through a reference.
- The value SHALL be of boolean kind on both providers, so SQL Server
  spells it as a bit.

## Capabilities

### Modified Capabilities

- `query-repl`: the computed standard fields.

## Impact

- `src/query/core/codegen/context.rs`, `dialect.rs`;
  `tests/fixtures/demo/expected.jsonl` (35 queries more compile);
  README and `docs/query-language-support.md`.
