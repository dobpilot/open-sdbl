## ADDED Requirements

### Requirement: Nested tabular-section projection is named
A field path followed by `.(…)` or `.*` asks for a tabular section as a
nested result inside one column. The compiler SHALL report an
unsupported-feature diagnostic that names that construct, because one SQL
statement returns no nested result.

#### Scenario: Nested column list
- **WHEN** `ВЫБРАТЬ Т.Состав.(Ссылка, НомерСтроки) ИЗ Справочник.X КАК Т`
  is compiled
- **THEN** the diagnostic says the nested tabular-section result is not
  supported and points at the construct

#### Scenario: Nested wildcard
- **WHEN** the projection is `Т.Состав.*`
- **THEN** the same diagnostic is reported
