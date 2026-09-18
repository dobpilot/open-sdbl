## ADDED Requirements

### Requirement: Report a parameter used as a source
A parameter written where a source is expected (`ИЗ &Таблица`, or after
a join keyword) SHALL be reported as an `UnsupportedFeature` diagnostic
positioned at the parameter and stating that a table passed as a
parameter is not supported.

#### Scenario: Value table parameter
- **WHEN** `ВЫБРАТЬ Т.Номенклатура ИЗ &ТаблицаТоваров КАК Т` is compiled
- **THEN** compilation fails with `UnsupportedFeature` at `&ТаблицаТоваров`
