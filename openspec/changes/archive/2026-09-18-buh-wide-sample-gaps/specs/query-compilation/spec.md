## MODIFIED Requirements

### Requirement: An alias written without `КАК`
A projection SHALL accept an alias written without `КАК`, naming the
output column exactly as the `КАК` form names it, and one word only. A
word that opens the next clause SHALL never be read as such an alias,
neither in a projection nor after a source. After `КАК`, any other word
SHALL be accepted as the alias, a keyword included — `КАК Конец`,
`КАК Порядок` — as the platform accepts it. A wildcard projection SHALL
keep refusing an alias in either form.

#### Scenario: A projection alias without `КАК`
- **WHEN** `ВЫБРАТЬ Т.Поле Имя ИЗ Справочник.X КАК Т` is compiled
- **THEN** the column is named `Имя`

#### Scenario: A clause keyword is not an alias
- **WHEN** `ВЫБРАТЬ 1 КАК Ч ИЗ Справочник.X КАК Т ИТОГИ СУММА(Ч) ПО ОБЩИЕ`
  is compiled
- **THEN** `ИТОГИ` opens the totals clause instead of naming the source

#### Scenario: A keyword after `КАК`
- **WHEN** `ВЫБРАТЬ Т.Дата КАК Конец ИЗ Документ.X КАК Т` is compiled
- **THEN** the column is named `Конец`

#### Scenario: A clause keyword after `КАК`
- **WHEN** `ВЫБРАТЬ Т.Дата КАК ИЗ Документ.X КАК Т` is compiled
- **THEN** the missing alias is diagnosed

#### Scenario: A wildcard keeps refusing an alias
- **WHEN** `ВЫБРАТЬ * Имя ИЗ Справочник.X КАК Т` is compiled
- **THEN** the alias is refused

### Requirement: Ordering a distinct statement
A statement with `РАЗЛИЧНЫЕ` SHALL order by its projected columns,
addressing them by position, because such a statement keeps only the
values it projects. A projected reference field SHALL be addressed by
the column the projection renders for it, whatever physical members the
reference has. An ordering field the projection does not carry SHALL
be refused with a diagnostic naming the reason.

#### Scenario: Ordering by a projection alias
- **WHEN** `ВЫБРАТЬ РАЗЛИЧНЫЕ Т.Наименование КАК Имя ИЗ Справочник.X КАК Т
  УПОРЯДОЧИТЬ ПО Имя` is compiled
- **THEN** the ordering addresses the projected column by its position

#### Scenario: Ordering by a projected reference of several types
- **WHEN** `ВЫБРАТЬ РАЗЛИЧНЫЕ Р.Регистратор КАК Ссылка ИЗ РегистрНакопления.X КАК Р
  УПОРЯДОЧИТЬ ПО Р.Регистратор` is compiled
- **THEN** the ordering addresses the rendered reference column by its
  position

#### Scenario: Ordering by a field outside the projection
- **WHEN** a distinct statement orders by a field it does not project
- **THEN** the query is refused
