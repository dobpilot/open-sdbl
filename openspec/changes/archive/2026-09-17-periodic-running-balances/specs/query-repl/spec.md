## ADDED Requirements

### Requirement: Running balances of a split table
On PostgreSQL and SQL Server 2012 and newer, `ОстаткиИОбороты` of an
accumulation or an accounting register split by a calendar period, the
recorder, the record, or — under `Авто` — by the split fields the
statement reads, SHALL answer its balance columns as running sums: the
active movements before `Конец` are bucketed by the grain, the
movements before `Начало` forming one bucket that sorts first and is
dropped after the window; `НачальныйОстаток` is the sum of the buckets
before the current one, `КонечныйОстаток` the sum up to it, partitioned
by the dimensions; the debit and credit parts of an accounting balance
are derived from those sums. Under `Авто` the grain SHALL be the record
when `НомерСтроки` is read, the recorder when `Регистратор` or `Период`
is read, otherwise the finest calendar level read; a statement reading
no balance column SHALL keep the relation without windows. On SQL
Server 2008 the balance columns of a split table SHALL be refused with
an `UnsupportedFeature` diagnostic naming the server.

#### Scenario: Balances by recorder
- **WHEN** `ВЫБРАТЬ О.Регистратор, О.КоличествоНачальныйОстаток, О.КоличествоКонечныйОстаток ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(&Н, &К, Авто, , ) КАК О`
  is compiled for PostgreSQL
- **THEN** the balances are `SUM(SUM(…)) OVER (PARTITION BY <dimensions>
  ORDER BY … ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)` and `…
  CURRENT ROW` over the record buckets, and the bucket before `&Н` is
  dropped

#### Scenario: Turnovers only
- **WHEN** the same table is read for `ПериодДень` and `КоличествоОборот`
- **THEN** the relation carries no window

#### Scenario: SQL Server 2008
- **WHEN** a split table's balance is read with the `Sql2008` dialect level
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
