## ADDED Requirements

### Requirement: Restriction condition in the platform's full form
A restriction condition SHALL be accepted in the platform's full form:
an optional leading `ТекущаяТаблица`, an optional alias after `КАК`, an
optional `ГДЕ`, then the condition. `ТекущаяТаблица` and the alias SHALL
qualify the fields of the restricted table, in a nested query of the
condition as well, while an unqualified field keeps resolving against
it. A join written between `ТекущаяТаблица` and `ГДЕ` SHALL be refused
with a `Restriction` diagnostic saying joins are not supported.

#### Scenario: Correlated key check
- **WHEN** the restriction is `ТекущаяТаблица ГДЕ ИСТИНА В (ВЫБРАТЬ ПЕРВЫЕ 1 ИСТИНА ИЗ Справочник.Y КАК К ГДЕ К.Объект = ТекущаяТаблица.Ссылка)`
- **THEN** the nested query compares with the restricted source's column

#### Scenario: Alias
- **WHEN** the restriction is `ТекущаяТаблица КАК Т ГДЕ Т.Организация = &Орг`
- **THEN** it compiles as `Организация = &Орг` would

#### Scenario: Join
- **WHEN** the restriction is `ТекущаяТаблица ЛЕВОЕ СОЕДИНЕНИЕ Справочник.Y КАК К ПО … ГДЕ …`
- **THEN** compilation fails with a `Restriction` diagnostic
