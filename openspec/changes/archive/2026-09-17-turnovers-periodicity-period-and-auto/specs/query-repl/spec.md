## ADDED Requirements

### Requirement: Accept the period and auto periodicities
The periodicity argument of `Обороты` and `ОстаткиИОбороты` SHALL accept
`Период`/`Period` and `Авто`/`Auto` and SHALL compile either like an
omitted periodicity: the table answers one row per combination of the
dimensions in use over the whole interval and exposes no `Период`
column. (`Период` is the platform's documented default; `Авто` splits by
the period fields a query reads, which the compiler refuses, so without
them it is the default too.)

#### Scenario: Turnovers for the whole period
- **WHEN** `РегистрНакопления.Продажи.Обороты(&Н, &К, Период, )` is read by товар
- **THEN** the SQL groups by товар only and applies the interval bounds

#### Scenario: Auto without period fields
- **WHEN** `РегистрНакопления.Продажи.ОстаткиИОбороты(&Н, &К, Авто, , )` is read
- **THEN** it compiles like the table without a periodicity
