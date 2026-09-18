## ADDED Requirements

### Requirement: Split by the fields the statement reads under Auto
Under the `Авто`/`Auto` periodicity, `Обороты` and `ОстаткиИОбороты`
SHALL expose `Период` (the record period), `ПериодСекунда`, `ПериодМинута`,
`ПериодЧас`, `ПериодДень`, `ПериодНеделя`, `ПериодДекада`, `ПериодМесяц`,
`ПериодКвартал`, `ПериодПолугодие`, `ПериодГод` (`SecondPeriod` …
`YearPeriod`: the beginning of that period of the record), `Регистратор`
and `НомерСтроки`, and SHALL treat them as dimensions of the relation:
the ones the statement reads split the rows, the others are summed away
like an unread dimension. `ОстаткиИОбороты` SHALL refuse its balance
columns with an `UnsupportedFeature` diagnostic when the statement reads
one of these split fields, and SHALL answer them otherwise; a period
completion method SHALL be accepted with `Авто`.

#### Scenario: Turnovers by month and recorder
- **WHEN** `ВЫБРАТЬ О.ПериодМесяц, О.Регистратор, О.СуммаОборот ИЗ РегистрНакопления.Продажи.Обороты(&Н, &К, Авто, ) КАК О` is compiled
- **THEN** the SQL groups by the beginning of the month of the record
  period and by the recorder columns

#### Scenario: Auto without split fields
- **WHEN** the statement reads dimensions and resources only
- **THEN** the SQL is the same as without a periodicity

#### Scenario: Balance with a split field
- **WHEN** `О.Регистратор` and `О.СуммаКонечныйОстаток` are read from `ОстаткиИОбороты(&Н, &К, Авто, ДвиженияИГраницыПериода, )`
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
