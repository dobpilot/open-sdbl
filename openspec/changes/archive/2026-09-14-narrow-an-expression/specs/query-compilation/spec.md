## ADDED Requirements

### Requirement: Narrow an expression to a metadata type
`ВЫРАЗИТЬ(<выражение> КАК <Вид>.<Объект>)` SHALL narrow a computed value:
a reference of that type keeps its value, a runtime-typed payload is
narrowed by its type prefix and answers `NULL` for another type, and a
value that is `NULL` whatever its type narrows to `NULL` of the named
type. Every other kind SHALL be refused, as the platform refuses it.

#### Scenario: Alternatives narrowed to their type
- **WHEN** `ВЫРАЗИТЬ(ВЫБОР … ТОГДА Т.Клиент ИНАЧЕ ЗНАЧЕНИЕ(…) КОНЕЦ КАК
  Справочник.Клиенты)` is compiled
- **THEN** the value is kept as a reference of that catalog

#### Scenario: Value that cannot hold the type
- **WHEN** a number is narrowed to a catalog
- **THEN** the compiler refuses it, as the platform does
