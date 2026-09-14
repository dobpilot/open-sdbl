## ADDED Requirements

### Requirement: Simple form of ВЫБОР
`ВЫБОР <выражение> КОГДА <значение> ТОГДА …` SHALL compile every
alternative as the comparison of the subject with the value of that
alternative, using the same rules as a comparison written in `ГДЕ`, so
that references, composite fields and type values answer alike. A subject
that is `NULL` SHALL match no alternative.

#### Scenario: Alternatives of a simple ВЫБОР
- **WHEN** `ВЫБОР Т.Цена КОГДА 10 ТОГДА "десять" ИНАЧЕ "прочее" КОНЕЦ` is
  compiled
- **THEN** rows whose price is ten answer the first value and every other
  row answers the alternative, as on the platform

#### Scenario: Type value as the subject
- **WHEN** the subject is `ТИПЗНАЧЕНИЯ(поле)` and an alternative is
  `ТИП(Справочник.X)`
- **THEN** the alternative matches the rows holding a reference to that
  catalog
