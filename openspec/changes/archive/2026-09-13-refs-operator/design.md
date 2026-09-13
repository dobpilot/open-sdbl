## Context

Measured on the platform (8.3.27, probe base, 2026-09-13) with a
composite attribute `Объект` (two catalogs plus a string), a
single-target attribute `Клиент`, and a dereferenced path:

- `Объект ССЫЛКА Справочник.Товары` is true only for rows holding a
  reference of that catalog; a string or an empty value gives false.
- `Клиент ССЫЛКА Справочник.Клиенты` is true for every row, including
  the empty reference: the type of a fixed-target field is known.
- `Клиент ССЫЛКА Справочник.Товары` (a type the field cannot hold) and
  `Цена ССЫЛКА Справочник.Товары` fail with `Несовместимые типы`.
- `Поставщик.Объект ССЫЛКА Справочник.Клиенты` works through the
  dereference join.

`ВЫРАЗИТЬ(… КАК Справочник.X)` already resolves the target object, its
database type number, the field's reference members, and the
admissibility of the target for a fixed-target field; the reference
type column is `_RTRef` (`is_reference_type_member`) and derived scopes
expose a runtime-typed payload column whose first four bytes are the
type number.

## Decisions

- `Expression::Refs { token, value, kind, object }` is parsed after the
  additive operand, before `ЕСТЬ`, `В`, and comparison operators. The
  keyword joins the contextual identifiers, so `Т.Ссылка`, `Ссылка КАК
  …`, and `ПРЕДСТАВЛЕНИЕ(Ссылка)` keep parsing.
- The operand must be a field path (direct or one dereference), as for
  `ВЫРАЗИТЬ`; other expressions are `Syntax`. Resolution reuses the
  cast machinery: target id and database type number, reference value
  member, optional type member, fixed targets.
- Rendering: with a type member, `(<alias>.<_RTRef> = 0x0000NNNN)`; a
  derived runtime-typed payload, `(SUBSTRING(payload, 1, 4) =
  0x0000NNNN)`; a fixed-target field whose target is the named table,
  the dialect's true predicate; a fixed-target field naming another
  table, `Syntax` (`field "X" cannot hold Справочник.Y`); a
  non-reference field, `Syntax` (`REFS argument must be a reference
  field`). The type tag column (`_TYPE`) is not consulted: a primitive
  value stores a zero `_RTRef`, so the equality is already false.
- The operator is a predicate; in value positions (`ВЫБОР КОГДА x
  ССЫЛКА … ТОГДА`) it compiles through the predicate path as `ЕСТЬ NULL`
  does. Source-free statements refuse it (`REFS requires FROM`).

## Risks / Trade-offs

- The plan proposed a constant false for a target outside the field's
  types; the platform rejects the query instead, and the diagnostic is
  the more useful behaviour, so the platform wins.
