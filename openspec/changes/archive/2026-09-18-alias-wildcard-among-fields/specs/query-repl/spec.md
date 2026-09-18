## MODIFIED Requirements

### Requirement: Alias wildcard of the only source
`<Псевдоним>.*` naming a source — by its alias or, without one, by its
object name — SHALL stand for every field of that source in the
metadata order, wherever in the projection list it is written: alone
it equals `*`, and next to named fields or in a joined statement it
adds the source's fields at its place, their labels made unique as any
repeated label is. Any other `<Имя>.*` keeps naming a tabular section.

#### Scenario: Alias wildcard
- **WHEN** `ВЫБРАТЬ Т.* ИЗ Справочник.X КАК Т` is compiled
- **THEN** the SQL equals that of `ВЫБРАТЬ * ИЗ Справочник.X КАК Т`

#### Scenario: Alias wildcard among fields in a join
- **WHEN** `ВЫБРАТЬ Т.Код КАК Код, Т.* ИЗ Справочник.X КАК Т ЛЕВОЕ СОЕДИНЕНИЕ Справочник.Y КАК Д ПО Т.Код = Д.Код`
  is compiled
- **THEN** the projection is `Код` followed by every field of `Т`, the
  repeated `Код` labelled uniquely
