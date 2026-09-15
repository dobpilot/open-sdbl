## ADDED Requirements

### Requirement: Present an aggregate
`ПРЕДСТАВЛЕНИЕ` MAY take an aggregate as its argument. Such a projection
SHALL count as an aggregated projection, so the branch aggregates like any
other, whether or not it groups. A non-reference aggregate SHALL answer its
own value as text; a reference aggregate SHALL answer through the
presentation protocol, as a reference expression does.

#### Scenario: Presented count
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(КОЛИЧЕСТВО(*)) ИЗ Справочник.Товары` is
  compiled
- **THEN** the branch aggregates and the column answers the count as text,
  as the platform answers

#### Scenario: Presented reference aggregate
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(МАКСИМУМ(Т.Клиент)) ИЗ Справочник.Товары
  КАК Т` is compiled
- **THEN** the column carries the greatest reference for the presentation
  protocol to resolve, which answers what the platform answers

#### Scenario: Presented aggregate of a group
- **WHEN** the same projection appears beside `СГРУППИРОВАТЬ ПО`
- **THEN** every group answers its own aggregate
