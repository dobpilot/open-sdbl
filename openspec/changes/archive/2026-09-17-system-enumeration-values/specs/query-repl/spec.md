## ADDED Requirements

### Requirement: Compile system enumeration values
`ЗНАЧЕНИЕ`/`VALUE` SHALL accept a two-segment path naming a system
enumeration and one of its values, bilingually, and SHALL compile it to
the number the platform stores for that value: `ВидДвиженияНакопления`
(`AccumulationRecordType`) with `Приход`/`Receipt` 0 and
`Расход`/`Expense` 1; `ВидДвиженияБухгалтерии` (`AccountingRecordType`)
with `Дебет`/`Debit` 0 and `Кредит`/`Credit` 1; `ВидСчета`
(`AccountType`) with `Активный`/`Active` 0, `Пассивный`/`Passive` 1 and
`АктивноПассивный`/`ActivePassive` 2. The expression SHALL be accepted
wherever a numeric literal is, including the conditions of virtual
tables, and an unknown enumeration or value SHALL be a `Syntax`
diagnostic naming it.

#### Scenario: Movement kind in a predicate
- **WHEN** `ГДЕ Т.ВидДвижения = ЗНАЧЕНИЕ(ВидДвиженияНакопления.Расход)` is compiled
- **THEN** the SQL compares the `_RecordKind` column with `1`

#### Scenario: Account kind
- **WHEN** `ВЫБОР КОГДА О.Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.АктивноПассивный) ТОГДА …` is compiled
- **THEN** the dereferenced `_Kind` column of the chart of accounts is
  compared with `2`

#### Scenario: Unknown value
- **WHEN** `ЗНАЧЕНИЕ(ВидСчета.Дебет)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic that names the value

### Requirement: Expose chart-of-accounts standard fields
A chart of accounts SHALL expose `Вид`/`Kind`, `Забалансовый`/`OffBalance`
and `Порядок`/`Order` as standard fields, also through a dereference.

#### Scenario: Off-balance accounts
- **WHEN** `ГДЕ Счета.Забалансовый` is compiled over `ПланСчетов.Управленческий`
- **THEN** the SQL reads the `_OffBalance` column
