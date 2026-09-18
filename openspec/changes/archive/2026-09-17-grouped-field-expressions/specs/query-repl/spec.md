## ADDED Requirements

### Requirement: Project expressions of grouped fields
In a statement with `СГРУППИРОВАТЬ ПО`, a projection that is neither a
key nor aggregated SHALL be accepted when every field it reads is named
by a grouping key or the projection is an expression a key spells;
otherwise it SHALL stay an `UnsupportedFeature` diagnostic.

#### Scenario: Negated grouped resource
- **WHEN** `ВЫБРАТЬ Д.Сумма, -Д.Сумма КАК Минус ИЗ … КАК Д СГРУППИРОВАТЬ ПО Д.Сумма`
  is compiled
- **THEN** the SQL groups by the resource column and projects its
  negation

#### Scenario: Ungrouped operand
- **WHEN** a projection reads a field no key names
- **THEN** the diagnostic names the projection
