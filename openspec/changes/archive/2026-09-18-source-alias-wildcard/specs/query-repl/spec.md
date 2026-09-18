## ADDED Requirements

### Requirement: Alias wildcard of the only source
When a statement has one source and its only projection is
`<Псевдоним>.*` naming that source — by its alias or, without one, by
its object name — the projection SHALL be every field of the source,
exactly as `*`; any other `<Имя>.*` keeps naming a tabular section.

#### Scenario: Alias wildcard
- **WHEN** `ВЫБРАТЬ Т.* ИЗ Справочник.X КАК Т` is compiled
- **THEN** the SQL equals that of `ВЫБРАТЬ * ИЗ Справочник.X КАК Т`
