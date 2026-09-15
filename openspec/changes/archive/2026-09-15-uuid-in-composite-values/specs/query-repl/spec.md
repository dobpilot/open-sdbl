## ADDED Requirements

### Requirement: Carry a unique identifier in a composite value
A value whose branches differ in type and include a unique identifier or
raw bytes SHALL compile, spreading the identifier over a member `_U` and
the bytes over a member `_B` of the composite value. The type tag of such
a branch SHALL be the `Null` tag, which is what the platform answers for
`ТИПЗНАЧЕНИЯ` of such a value. Branches of other types SHALL write the
zero of those members, as they do for every other member.

The layout is an extension of what 1C stores: the platform keeps a
composite value over `_TYPE/_L/_N/_T/_S/_RTRef/_RRRef` and has no binary
member, because such a value never reaches a table.

#### Scenario: Identifier beside a reference
- **WHEN** `ВЫБОР КОГДА Т.Цена > 5 ТОГДА УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Т.Ссылка)
  ИНАЧЕ Т.Клиент КОНЕЦ` is compiled
- **THEN** the value carries the identifier in `_U`, the reference in the
  payload, and the row's type tag says which branch answered

#### Scenario: Stored binary field beside a reference
- **WHEN** a branch reads a field stored as raw bytes and another reads a
  reference
- **THEN** the bytes are carried in `_B` and the value compiles

#### Scenario: Type of such a value
- **WHEN** `ТИПЗНАЧЕНИЯ` is applied to that value
- **THEN** the identifier rows answer the `Null` type, as the platform
  answers
