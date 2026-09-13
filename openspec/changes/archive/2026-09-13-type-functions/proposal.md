## Why

1C queries inspect composite attributes with `ТИПЗНАЧЕНИЯ(Поле) =
ТИП(Справочник.Номенклатура)`, filter empty composite values with
`Поле = НЕОПРЕДЕЛЕНО`, and fill `ВЫБОР` branches with `НЕОПРЕДЕЛЕНО`.
None of the three is a keyword today, so such queries fail at the
lexer, and the `_TYPE` member of a composite field is never read.

## What Changes

- The lexer SHALL recognize `ТИП`/`TYPE`, `ТИПЗНАЧЕНИЯ`/`VALUETYPE`,
  and `НЕОПРЕДЕЛЕНО`/`UNDEFINED` as keywords; the first two stay
  contextual identifiers.
- The parser SHALL accept `ТИП(Строка | Число | Дата | Булево |
  <Вид>.<Объект>)`, `ТИПЗНАЧЕНИЯ(<выражение>)`, and the literal
  `НЕОПРЕДЕЛЕНО` in expression positions.
- A type value SHALL be a five-byte binary: the platform's `_TYPE` tag,
  measured on the platform (`0x01` undefined, `0x02` boolean, `0x03`
  number, `0x04` date, `0x05` string, `0x08` reference, and `0x00` for
  the type of `NULL`), followed by the four-byte `RTRef` table number
  (zero for non-references). `ColumnKind` gains `Type` for such columns and
  `Undefined` for a column made of the literal alone; the console
  prints type values by name (`Справочник.Товары`, `Строка`, …).
- `ТИПЗНАЧЕНИЯ` of a composite field reads `_TYPE` and `_RTRef`, of a
  runtime-typed derived column reads the payload prefix, of every other
  expression is a constant of its kind guarded by `IS NULL`, and of a
  `NULL` value is the `NULL` type, as measured on the platform.
  Comparisons and `В (…)` compare the encoded values, which are never
  SQL `NULL`, so `<>` answers as the platform does.
- `НЕОПРЕДЕЛЕНО` compiles to `NULL`; comparing a value with it compares
  its type with the undefined type, so a composite field holding the
  undefined value matches and nothing else does; comparing a value whose
  kind cannot hold it is a constant false (`<>`: true) predicate, as the
  platform returns no rows rather than an error; in `ВЫБОР`/`ЕСТЬNULL` it is compatible with every kind
  like `NULL`, so the result cannot tell `Неопределено` from `NULL`.
- `ТИП` of anything but the four primitives and a metadata object with
  a type number (for example `УникальныйИдентификатор`) SHALL be a
  `Syntax` diagnostic, as the platform rejects it too. `ТИПЗНАЧЕНИЯ`
  of a UUID, binary, or unclassified expression SHALL be
  `UnsupportedFeature`. Grouping and ordering by `ТИПЗНАЧЕНИЯ` are out
  of scope.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: three new bilingual keywords.
- `query-compilation`: two new `ColumnKind` variants and the public
  `TypeValue` codec.
- `query-repl`: the `ТИП`, `ТИПЗНАЧЕНИЯ`, and `НЕОПРЕДЕЛЕНО`
  expressions and the console rendering of type values.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`, `resolve.rs`
  (`ColumnKind`), new `src/query/core/types.rs`, `codegen/expression.rs`,
  `codegen/sources.rs`, `codegen/select.rs`; CLI cell rendering and
  completion; README and `docs/query-language-support.md`.
