## ADDED Requirements

### Requirement: Compile the balance-and-turnovers table
`РегистрНакопления.X.ОстаткиИОбороты(Начало, Конец, Периодичность,
МетодДополненияПериодов, Условие)` SHALL compile for a balance register
and SHALL expose, per resource, `НачальныйОстаток`, `Приход`, `Расход`,
`Оборот` and `КонечныйОстаток` beside the register's dimensions. The
opening balance SHALL be the signed movement before `Начало`, the
receipts and expenses the movements of `[Начало, Конец)` split by record
kind, the turnover their difference, and the closing balance the sum of
the opening balance and the turnover. The five columns SHALL be summed
over the dimensions the statement never reads, like every register
table. A turnover-only register SHALL be refused. The periodicity and
the period completion method SHALL be `UnsupportedFeature` diagnostics.

#### Scenario: Whole register
- **WHEN** `ВЫБРАТЬ О.Товар, О.КоличествоПриход, О.КоличествоКонечныйОстаток ИЗ РегистрНакопления.Продажи.ОстаткиИОбороты КАК О`
  is executed
- **THEN** there is one row per товар with its receipts and closing
  balance

#### Scenario: Interval
- **WHEN** the table is read over `[2024-02-01, 2024-05-01)`
- **THEN** the opening balance holds the movements before February and
  the closing balance adds the turnover of the interval

#### Scenario: Periodicity
- **WHEN** a periodicity is given
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
