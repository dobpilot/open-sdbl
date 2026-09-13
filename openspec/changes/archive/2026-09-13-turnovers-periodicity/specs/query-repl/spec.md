## ADDED Requirements

### Requirement: Compile turnovers by period
`РегистрНакопления.X.Обороты(Начало, Конец, Периодичность, Условие)` SHALL
accept a calendar periodicity from `Секунда` to `Год`, written as a bare
period name, and SHALL group the turnovers by the beginning of the period
each record falls into, exposing it as the `Период` field. The grouping
SHALL apply even when the statement never reads `Период`, as the platform
answers one row per period. `Регистратор` and `Запись` SHALL be an
`UnsupportedFeature` diagnostic.

#### Scenario: Monthly turnovers
- **WHEN** `ВЫБРАТЬ О.Период, О.Товар, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты(, , Месяц, ) КАК О`
  is executed
- **THEN** there is one row per month and товар, the period being the
  first day of the month

#### Scenario: Period never read
- **WHEN** only the resource is selected from a monthly turnover table
- **THEN** there is still one row per month

#### Scenario: Recorder periodicity
- **WHEN** the periodicity is `Регистратор`
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
