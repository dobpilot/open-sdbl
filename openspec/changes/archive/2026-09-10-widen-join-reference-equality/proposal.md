## Why

A join condition that compares a fixed reference with a runtime-typed
column of a derived source or temporary table (`ВходящийДокумент.Ссылка =
ОтветПереадресовавшему.Ссылка`) compiles to `"_idrref" = "Ссылка"`: a
16-byte value against a 20-byte `RTRef ‖ RRRef` payload that can never be
equal, so an outer join silently yields no matches. The reverse shape, a
composite field against a payload column (`СвязьОтправленОтвет.Объект =
ПоследнийОтправленОтвет.Документ`), fails with "fixed reference field has
no unique SchemaStorage target". `ГДЕ` and `В (ВЫБРАТЬ …)` already widen
such operands; join conditions do not.

## What Changes

- A join equality between reference operands of different width SHALL
  widen the narrower side to the `RTRef ‖ RRRef` payload: a fixed reference
  becomes `<type> ‖ id`, a composite field becomes `_RTRef ‖ _RRRef`, and a
  payload column stays as it is.
- The widened expression serves as the anchor marker of a transposed
  `ПОЛНОЕ СОЕДИНЕНИЕ`.
- No syntax, API, or console change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: join equality widening.

## Impact

- `src/query/core/codegen/select.rs` (`compile_join_field_equality` and
  the compound/fixed helper), tests, `docs/query-language-support.md`.
- Independent of the two following changes; archive first.
