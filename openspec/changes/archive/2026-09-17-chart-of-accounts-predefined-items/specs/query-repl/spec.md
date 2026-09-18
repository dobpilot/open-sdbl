## ADDED Requirements

### Requirement: Resolve predefined accounts
`ЗНАЧЕНИЕ`/`VALUE` SHALL accept a `ПланСчетов` object and SHALL resolve
the named predefined account through the chart's `_PredefinedID` column,
as it does for a catalog. Charts of characteristic types and of
calculation types SHALL stay refused until their resources are measured.

#### Scenario: Predefined account
- **WHEN** `ГДЕ О.Счет = ЗНАЧЕНИЕ(ПланСчетов.Управленческий.ПрочиеРасходы)` is compiled
- **THEN** the SQL selects `_IDRRef` of the chart's table by
  `_PredefinedID` equal to the stable identifier of `ПрочиеРасходы`
