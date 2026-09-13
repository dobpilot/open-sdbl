## ADDED Requirements

### Requirement: Recognize the type keywords bilingually
The lexer SHALL classify `ТИП`/`TYPE`, `ТИПЗНАЧЕНИЯ`/`VALUETYPE`, and
`НЕОПРЕДЕЛЕНО`/`UNDEFINED` case-insensitively as three keyword kinds
whose stable display names are `TYPE`, `VALUETYPE`, and `UNDEFINED`,
and the exhaustive keyword table test SHALL include every spelling. The
parser SHALL treat `ТИП` and `ТИПЗНАЧЕНИЯ` as contextual identifiers,
so attributes named `Тип` keep parsing in field positions.

#### Scenario: Attribute named Тип
- **WHEN** input contains `ВЫБРАТЬ Т.Тип ИЗ Справочник.Товары КАК Т ГДЕ ТИПЗНАЧЕНИЯ(Т.Тип) = ТИП(Строка)`
- **THEN** `Т.Тип` is a field path and the two function keywords are
  recognized
