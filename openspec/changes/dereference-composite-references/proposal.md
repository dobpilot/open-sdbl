## Why

Registers of links, subjects, and states hold composite reference fields
(`Объект`, `СвязанныйОбъект`) declared as any reference, and platform
queries dereference them directly: `СвязьВОтветНа.СвязанныйОбъект.
РегистрационныйНомер`. The compiler dereferences only fields with one
fixed SchemaStorage target and rejects composite fields and runtime-typed
derived columns with "has no unique SchemaStorage reference target".

## What Changes

- A one-hop dereference through a composite reference field, or through a
  runtime-typed column of a derived source or temporary table, SHALL join
  every candidate target that has the named attribute with a type-guarded
  `LEFT JOIN` and SHALL select the attribute with a `ВЫБОР` over the
  reference type.
- Candidates are the field's declared targets when SchemaStorage lists
  them; otherwise every reference-kind metadata object whose fields
  include the attribute, as the platform does for `ЛюбаяСсылка`. More than
  32 candidates is a diagnostic advising `ВЫРАЗИТЬ`.
- The resulting kind is the common kind of the attribute across targets:
  variants must agree, references widen to a runtime-typed payload.
- Presentation of the dereferenced value is not part of this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: composite dereference; nested-source rule relaxed.

## Impact

- `src/query/core/codegen/context.rs` (multi-target dereference planning,
  `ResolvedPath` expression override), `dialect.rs` (payload split
  helpers), `select.rs`/`expression.rs` where `sql_column` and kinds are
  consumed, tests, README, `docs/query-language-support.md`.
- Depends on `dereference-in-join-conditions` so that composite
  dereferences also work in `ПО`.
