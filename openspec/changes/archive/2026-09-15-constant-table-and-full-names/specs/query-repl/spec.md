## ADDED Requirements

### Requirement: Read a single constant as its own table
A source written as `Константа.<Имя>` / `Constant.<Name>` SHALL expose the
stored value under the field name `Значение` / `Value`, not under the name
of the constant, because that is the name the platform answers to. The
field SHALL behave like any other field of the source: it SHALL project,
filter, group, order, dereference when it holds a reference, and carry the
column kind of its stored type. The separator predicates of the constant's
table SHALL apply as they do to any other source.

#### Scenario: Value of a constant
- **WHEN** `ВЫБРАТЬ К.Значение ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** the stored value column of the constant's table is projected,
  and the result matches the platform

#### Scenario: Constant addressed by its own name
- **WHEN** `ВЫБРАТЬ К.ОсновнойТовар ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** compilation fails with an unknown-field diagnostic, as the
  platform reports "Поле не найдено"

#### Scenario: Dereferenced constant value
- **WHEN** the constant stores a reference and
  `ВЫБРАТЬ К.Значение.Наименование ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** the reference is joined and its description is projected

### Requirement: Qualify a source by its full metadata name
A field MAY be qualified by the full metadata name of a source that was
written without an alias, as in `ВЫБРАТЬ Справочник.Товары.Наименование ИЗ
Справочник.Товары`. When the source carries an alias, the full name SHALL
NOT resolve, because the alias replaces the name — the platform reports
"Поле не найдено" for that shape.

#### Scenario: Unaliased source qualified by its name
- **WHEN** `ВЫБРАТЬ Справочник.Товары.Наименование ИЗ Справочник.Товары` is
  compiled
- **THEN** the field resolves against that source and the result matches
  the platform

#### Scenario: Aliased source addressed by its name
- **WHEN** `ВЫБРАТЬ Т.Наименование ИЗ Справочник.Товары КАК Т ГДЕ
  Справочник.Товары.Цена > 0` is compiled
- **THEN** compilation fails, because the alias replaces the name

#### Scenario: Full name inside a subquery condition
- **WHEN** a correlated subquery selects from `Задача.ЗадачаИсполнителя`
  without an alias and its condition names
  `Задача.ЗадачаИсполнителя.Выполнена`
- **THEN** the field resolves against that source
