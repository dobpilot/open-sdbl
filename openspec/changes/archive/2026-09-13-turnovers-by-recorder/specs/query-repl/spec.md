## ADDED Requirements

### Requirement: Compile turnovers by recorder
`РегистрНакопления.X.Обороты` SHALL accept `Регистратор` and `Запись` as
its periodicity. `Регистратор` SHALL group the turnovers by the period
and the recorder of the records and SHALL expose the `Период` and
`Регистратор` fields; `Запись` SHALL additionally group by and expose
`НомерСтроки`, so each register record answers its own row. Neither
grouping SHALL be dropped when the statement does not read the columns.
`НомерСтроки` SHALL stay unavailable under `Регистратор`, as on the
platform.

#### Scenario: Turnovers of each document
- **WHEN** `ВЫБРАТЬ О.Период, О.Регистратор, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты(, , Регистратор, ) КАК О`
  is executed
- **THEN** there is one row per document and dimension combination in use

#### Scenario: Each record
- **WHEN** the periodicity is `Запись`
- **THEN** `НомерСтроки` is available and each register record answers
  its own row
