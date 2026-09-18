## ADDED Requirements

### Requirement: Dereferences of grouping keys
In a statement with `СГРУППИРОВАТЬ ПО`, a projection or a scalar
operand whose path extends a key written as a field path —
`Сотрудник.Наименование` over the key `Сотрудник` — SHALL be accepted,
and every column it reads SHALL be added to the `GROUP BY` list. An
`УПОРЯДОЧИТЬ ПО` key of a grouped statement SHALL be accepted when it is
a grouping key, a dereference of one (its columns joining the grouping)
or an expression containing an aggregate; other unprojected fields SHALL
stay an `UnsupportedFeature` diagnostic.

#### Scenario: Projected dereference
- **WHEN** `ВЫБРАТЬ p.Орг, p.Орг.Код … СГРУППИРОВАТЬ ПО p.Орг` is compiled
- **THEN** the SQL groups by the key column and the joined code column

#### Scenario: Ordering by an aggregate
- **WHEN** `… СГРУППИРОВАТЬ ПО p.Орг УПОРЯДОЧИТЬ ПО МАКСИМУМ(p.Код) УБЫВ` is compiled
- **THEN** the SQL orders by `MAX(...) DESC`
