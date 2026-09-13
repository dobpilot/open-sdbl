## ADDED Requirements

### Requirement: Compile type literals and the value-type function
The compiler SHALL accept `ТИП(Строка | Число | Дата | Булево |
<Вид>.<Объект>)` / `TYPE(String | Number | Date | Boolean | …)` as a
constant of kind `Type` and `ТИПЗНАЧЕНИЯ(<выражение>)` /
`VALUETYPE(…)` as an expression of kind `Type`. A type value SHALL be
the five-byte encoding of `TypeValue`. `ТИПЗНАЧЕНИЯ` of a composite
field SHALL read the `_TYPE` member and, when the tag says the value is
a reference, the table number from the `_RTRef` member or from the
field's single reference target; of a composite field that reached the
query through a derived source, the `_TYPE` column that source projects
next to the reference payload; of a runtime-typed payload without such a
column, the payload prefix; of a field, literal, bound parameter, or
expression of a primitive or fixed reference kind, the constant type of
that kind; and of a `NULL` value or an unbound parameter, the `NULL`
type, so the result is never SQL `NULL`. Comparisons and `В (…)` SHALL
compare the encoded values. `ТИП` of any other argument SHALL be a
`Syntax` diagnostic; `ТИПЗНАЧЕНИЯ` of a UUID, binary, or unclassified
expression SHALL be an `UnsupportedFeature` diagnostic; both work in
source-free statements when the argument does. The console SHALL
render a `Type` column by name: `Null`, `Неопределено`, `Булево`,
`Число`, `Строка`, `Дата`, or `<Вид>.<Имя>` of the referenced object.

#### Scenario: Composite attribute
- **WHEN** `ГДЕ ТИПЗНАЧЕНИЯ(Т.Объект) = ТИП(Справочник.Товары)` is
  compiled for a composite attribute
- **THEN** the predicate compares the encoded type, built from
  `_Fld<N>_TYPE` and `_Fld<N>_RTRef`, with `0x0800000035`

#### Scenario: Projected type
- **WHEN** `ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Т.Объект), ТИПЗНАЧЕНИЯ(Т.Цена), ТИП(Строка)`
  is compiled
- **THEN** the columns are of kind `Type`, the first reads the members,
  the second is `0x0300000000` unless the price is `NULL`, and the third
  is the constant `0x0500000000`

#### Scenario: Rejected argument
- **WHEN** `ТИП(УникальныйИдентификатор)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic

### Requirement: Compile the undefined literal
The compiler SHALL accept `НЕОПРЕДЕЛЕНО` / `UNDEFINED` as a literal of
kind `Undefined` rendered as SQL `NULL`. `<выражение> = НЕОПРЕДЕЛЕНО`
SHALL compare the value's type with the undefined type, so a composite
field holding the undefined value matches and no other value does;
comparing an expression whose kind cannot hold the undefined value SHALL
be a constant false predicate (`<>`: true), as the platform returns no
rows rather than an error.
In `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ` the literal SHALL be
compatible with every kind like `NULL`; a column made only of the
literal SHALL report kind `Undefined`.

#### Scenario: Composite filter
- **WHEN** `ГДЕ Т.Объект = НЕОПРЕДЕЛЕНО` is compiled
- **THEN** generated SQL compares the encoded type of the field with
  `0x0100000000`

#### Scenario: Fixed field
- **WHEN** `ГДЕ Т.Цена = НЕОПРЕДЕЛЕНО` is compiled
- **THEN** generated SQL contains a false predicate and no diagnostic
