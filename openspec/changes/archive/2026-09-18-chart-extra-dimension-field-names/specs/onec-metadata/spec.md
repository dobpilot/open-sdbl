## ADDED Requirements

### Requirement: Extra-dimension kind table field names
The standard columns of a chart's extra-dimension kinds table SHALL be
addressable as `ВидСубконто` (`DimKind`) and `ТолькоОбороты`
(`TurnoverOnly`) beside `НомерСтроки`.

#### Scenario: Kinds of an account
- **WHEN** `ВЫБРАТЬ ВС.ВидСубконто, ВС.ТолькоОбороты ИЗ ПланСчетов.Хозрасчетный.ВидыСубконто КАК ВС` is compiled
- **THEN** the columns `_dimkindrref` and `_turnoveronly` are projected
