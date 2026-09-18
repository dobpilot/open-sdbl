## ADDED Requirements

### Requirement: Expanded balances of the accounting register
`Остатки` SHALL expose `<Ресурс>РазвернутыйОстатокДт` and
`<Ресурс>РазвернутыйОстатокКт`, and `ОстаткиИОбороты` without a
periodicity `<Ресурс>НачальныйРазвернутыйОстатокДт/Кт` and
`<Ресурс>КонечныйРазвернутыйОстатокДт/Кт`: per account, dimensions and
extra dimensions the positive part of the balance and the negated
negative part, which the outer aggregation sums over the dimensions the
statement does not read. A periodic `ОстаткиИОбороты` SHALL refuse them
with an `UnsupportedFeature` diagnostic. The period completion method
SHALL be accepted without a periodicity.

#### Scenario: Expanded balance by account
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаРазвернутыйОстатокДт ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, , , ) КАК О`
  is compiled
- **THEN** the inner aggregation projects the positive part of the
  group sum and the outer sums it by account

#### Scenario: Periodic table
- **WHEN** the same column is read from `ОстаткиИОбороты(&Н, &К, МЕСЯЦ, , , , )`
- **THEN** the diagnostic is `UnsupportedFeature`
