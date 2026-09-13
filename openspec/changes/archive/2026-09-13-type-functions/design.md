## Context

Measured on the platform (8.3.27, probe base, catalog `Товары` with the
composite attribute `Объект` of `Справочник.Товары | Справочник.Клиенты |
Строка(10)`, 2026-09-13):

- `ТИПЗНАЧЕНИЯ` returns a type for every value: `Строка`, `Число`,
  `Дата`, `Булево`, the catalog of a reference (also for the empty
  reference of a fixed-target field), `Неопределено` for an undefined
  composite value, and the type `Null` for `NULL` — both for the `NULL`
  literal and for a field of a missed `LEFT JOIN`.
- `ТИПЗНАЧЕНИЯ(x) = ТИП(…)`, `<>`, and `В (ТИП(…), ТИП(…))` work in
  `ВЫБОР` and `ГДЕ`; `ТИПЗНАЧЕНИЯ(x) = ТИПЗНАЧЕНИЯ(y)` compares two
  fields; parameters and nested-query columns are accepted.
- `ТИП(НЕОПРЕДЕЛЕНО)` and `ТИП(УникальныйИдентификатор)` fail with
  «Таблица не найдена»: only the four primitives and metadata objects
  are type literals.
- `Т.Объект = НЕОПРЕДЕЛЕНО` selects the undefined value, `<>` every
  other value including the empty reference; `Т.Цена = НЕОПРЕДЕЛЕНО`
  and `Т.Клиент = НЕОПРЕДЕЛЕНО` are not errors and select nothing.
- `НЕОПРЕДЕЛЕНО` projects as a column, fills `ВЫБОР` branches and the
  `ЕСТЬNULL` fallback; grouping and ordering by `ТИПЗНАЧЕНИЯ` are
  allowed (not implemented here).

## Decisions

### Encoding

A type value is five bytes: the `_TYPE` tag the platform stores in
composite members followed by the big-endian `RTRef` number (zeros for
non-references), and `0x00` with zeros for the type of `NULL`, which has
no storage tag. The tags were measured by writing one value of each type
into a five-type composite attribute of the probe catalog and reading
the `_TYPE` member back: `0x01` undefined, `0x02` boolean, `0x03`
number, `0x04` date, `0x05` string, `0x08` reference. The public
`TypeValue` enum (`Null`, `Undefined`, `Boolean`, `Number`, `String`,
`Date`, `Reference(u32)`) encodes and decodes it and names the type via
`MetadataSnapshot::object_id_by_database_type`; the console prints the
name (`Справочник.Товары`, `Строка`, …) for columns of kind `Type`.
Both dialects concatenate binaries (`||` / `+`), so the value is built
in SQL without casts.

### `ТИПЗНАЧЕНИЯ`

- Composite field with `_TYPE` and `_RTRef`: `COALESCE(CASE WHEN t =
  0x08 THEN t || r ELSE t || 0x00000000 END, <null type>)`. A composite
  with a single reference target has no `_RTRef` member, so `r` is that
  target's number; a composite without any reference alternative needs
  no `CASE` at all.
- A composite field read from a derived source: that source projects the
  `_TYPE` member as `<name>_TYPE` next to the reference payload, so the
  companion column plays the part of `t` and `SUBSTRING(p, 1, 4)` that
  of `r`.
- A runtime-typed payload without such a companion (a universal
  reference): `COALESCE(0x08 || SUBSTRING(p, 1, 4), <null type>)`.
- Fixed reference field: `CASE WHEN c IS NULL THEN <null type> ELSE
  <0x08 ‖ number> END`; primitive kinds likewise with their tag; `NULL`
  literal and `NULL` parameter: the null type; `НЕОПРЕДЕЛЕНО`: the
  undefined type; other literals and parameters: constants.
- `Uuid`, `Binary`, `Unknown` kinds: `UnsupportedFeature`.
- An unbound parameter (the preparation pass, before values are known)
  compiles to `NULL`, so its type is the `NULL` type; the bound pass
  then sees the real kind.
- Comparisons compare the encoded values. The encoding is injective and
  never SQL `NULL`, so `=`, `<>`, and `В (…)` answer as the platform
  does, including for a missing `LEFT JOIN` row. Member predicates
  would be sargable but would make `<>` return `NULL` there, which the
  platform does not.

### `ТИП`

`ТИП(Строка|Число|Дата|Булево)` and the English `String|Number|Date|
Boolean` are constants; `ТИП(<Вид>.<Объект>)` resolves the object like
`ЗНАЧЕНИЕ` and uses its type number; anything else is `Syntax`.

### `НЕОПРЕДЕЛЕНО`

The literal is SQL `NULL` of kind `Undefined`, a wildcard kind like
`Null` for `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ`. `common_kind` reports
`Undefined` when every operand is a wildcard and one of them is the
literal, so `ВЫБРАТЬ НЕОПРЕДЕЛЕНО` is typed. A comparison with the
literal is the comparison of the value's type with the undefined type,
which is one code path for every operand: a composite field matches only
when its tag is `0x01`, and a value whose kind cannot hold the undefined
value (binary, UUID, a type value, an unclassified catalog type) short
circuits to a constant false (`<>`: true) predicate, following the
platform's silent empty result. The result cannot tell `Неопределено` from
`NULL` in mixed expressions; documented.

## Risks / Trade-offs

- Type ordering and grouping are not implemented; the platform orders
  types by an internal sequence.
- Verified 2026-09-13 against the platform on the probe base: the type
  of every value of a five-type composite attribute, of primitive and
  reference fields, of a field behind a missed `LEFT JOIN`, of a
  dereferenced composite, of literals and a bound parameter, of a
  composite read through a nested query, the comparisons with `ТИП(…)`
  in `ГДЕ` and `ВЫБОР`, and the `НЕОПРЕДЕЛЕНО` comparisons all match
  row for row. `ТИП(НЕОПРЕДЕЛЕНО)` and `ТИП(УникальныйИдентификатор)`
  fail on both sides.
- A result cannot distinguish `Неопределено` from `NULL`, because the
  literal is SQL `NULL`; the platform prints them differently.
