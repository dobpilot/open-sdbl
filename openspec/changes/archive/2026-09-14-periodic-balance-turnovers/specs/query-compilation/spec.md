## ADDED Requirements

### Requirement: Periodicity of ОстаткиИОбороты
`ОстаткиИОбороты` SHALL accept a calendar periodicity, group the
movements of the interval into those periods and expose `Период`, which is
what the platform answers where no balance column is read. A period
completion method SHALL be accepted only together with a periodicity, as
both methods answer the same rows there. A balance column of a periodic
table SHALL be refused, because its value is a running sum over the
periods before it, which the platform accumulates outside SQL.

#### Scenario: Turnovers by month
- **WHEN** `ВЫБРАТЬ О.Период, О.КоличествоОборот ИЗ
  РегистрНакопления.X.ОстаткиИОбороты(, , Месяц, ) КАК О` is compiled
- **THEN** the relation groups the movements by month and answers one row
  per month with movements

#### Scenario: Balance of a periodic table
- **WHEN** the statement reads `КоличествоНачальныйОстаток` of a periodic
  table
- **THEN** the compiler reports that a periodic table answers no balance
  column
