## Why

`<поле> ССЫЛКА Справочник.X` is how 1C queries test the type of a
composite reference (`Регистратор ССЫЛКА Документ.РеализацияТоваровУслуг`,
`Владелец ССЫЛКА Справочник.Контрагенты`). The compiler has no such
operator; the workaround `ВЫРАЗИТЬ(… КАК Справочник.X) ЕСТЬ НЕ NULL`
is longer and differs for empty references.

## What Changes

- The lexer SHALL recognize `ССЫЛКА`/`REFS` as a keyword that stays a
  contextual identifier, because `Ссылка` is also the standard reference
  field name.
- The parser SHALL accept `<выражение> ССЫЛКА <Вид>.<Объект>` at the
  comparison level, like `ПОДОБНО`.
- The compiler SHALL render the test on the reference's type column:
  `_RTRef = <номер типа>` for a composite field, `SUBSTRING(payload,
  1, 4) = <номер типа>` for a runtime-typed nested-query column, and a
  constant true predicate for a fixed-target reference of that table,
  which the platform also treats as true for the empty reference. A
  target the field cannot hold and a non-reference operand SHALL be
  `Syntax` diagnostics, matching the platform's `Несовместимые типы`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: one new bilingual keyword.
- `query-repl`: the `ССЫЛКА` operator.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/expression.rs`, `codegen/sources.rs`, `codegen/select.rs`;
  CLI completion; README and `docs/query-language-support.md`.
