## ADDED Requirements

### Requirement: Accumulation balance and turnovers by recorder
`РегистрНакопления.<Имя>.ОстаткиИОбороты` SHALL accept `Регистратор` and
`Запись` as the periodicity: one row per dimensions combination, record
period and recorder — and line number for `Запись` — with the receipts,
expenses and turnover of the bucket and the opening and closing balances
as running sums over the buckets before it, on a server with window
frames; SQL Server 2008 SHALL refuse the balance columns as for a
calendar periodicity.

#### Scenario: By recorder
- **WHEN** `ОстаткиИОбороты(&Н, &К, Регистратор, , )` is compiled and
  `Регистратор` and `СуммаНачальныйОстаток` are read
- **THEN** the relation groups by the record period and the recorder and
  the opening balance is a window sum over the earlier buckets
