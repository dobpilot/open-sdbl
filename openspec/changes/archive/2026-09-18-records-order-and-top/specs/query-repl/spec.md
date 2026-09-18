## ADDED Requirements

### Requirement: First records of the accounting records table
`РегистрБухгалтерии.<Имя>.ДвиженияССубконто(Начало, Конец, Условие,
Порядок, Первые)` SHALL keep the first `Первые` records — a number
literal or a parameter bound to a number, rendered as `LIMIT` on
PostgreSQL and `TOP (N)` on SQL Server — ordered by `Порядок`: record
fields ascending, singly or as a tuple, or the record order (period,
recorder, line number) when `Порядок` is absent or a parameter bound to
`NULL`. `Порядок` without `Первые` SHALL change nothing. Another
`Первые` or `Порядок` SHALL be an `UnsupportedFeature` diagnostic.

#### Scenario: First record by period
- **WHEN** `ДвиженияССубконто(&Н, &К, , Период, 1)` is compiled for PostgreSQL
- **THEN** the relation ends with `ORDER BY` the period column and `LIMIT 1`

#### Scenario: First five in record order
- **WHEN** `ДвиженияССубконто(&Н, &К, Организация = &О, , 5)` is compiled
- **THEN** the relation orders by period, recorder and line number and
  keeps five rows
