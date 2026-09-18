## ADDED Requirements

### Requirement: Compile accounting balances
`РегистрБухгалтерии.<Имя>.Остатки(Период, УсловиеСчета, [Субконто],
Условие)` of a register with correspondence SHALL answer, per account
and dimensions in use, `<Ресурс>Остаток` — the debit rows minus the
credit rows of the active records before `Период` (all records when it
is omitted) — and `<Ресурс>ОстатокДт`/`<Ресурс>ОстатокКт` as the
positive part and the negated negative part of that balance at the grain
the statement reads, computed after unread dimensions are summed away,
and SHALL drop combinations whose every balance is zero. The
`Субконто` argument SHALL be absent for a register without extra
dimensions.

#### Scenario: Balance parts after pruning
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаОстатокДт ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д) КАК О`
  is compiled
- **THEN** the organization is summed away first and the debit part is
  the positive part of the summed balance, not a sum of the parts

#### Scenario: Zero balance
- **WHEN** every resource balance of one combination is zero
- **THEN** the combination is absent from the result

### Requirement: Compile accounting balances and turnovers
`РегистрБухгалтерии.<Имя>.ОстаткиИОбороты(Начало, Конец, Периодичность,
МетодДополненияПериодов, УсловиеСчета, [Субконто], Условие)` SHALL
answer, per account and dimensions in use, `<Ресурс>НачальныйОстаток`
(the balance of the records before `Начало`), `<Ресурс>Оборот`,
`<Ресурс>ОборотДт`, `<Ресурс>ОборотКт` of `[Начало, Конец)`, and
`<Ресурс>КонечныйОстаток`, with the debit and credit parts of both
balances derived at the grain read. A calendar or record periodicity
SHALL split the rows and refuse the balance columns; `Авто` SHALL expose
the split fields as dimensions and refuse the balance columns only when
one of them is read; the completion method follows the accumulation
table's rules.

#### Scenario: Balances by account
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаНачальныйОстаток, О.СуммаКонечныйОстатокКт ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, , , Счет = &Счет) КАК О`
  is compiled
- **THEN** the opening balance sums the records before `&Н`, the closing
  balance every record before `&К`, and the credit part is derived from
  the closing balance

#### Scenario: Auto with a balance
- **WHEN** the table is read under `Авто` with `Организация` and
  `СуммаКонечныйОстаток`
- **THEN** it compiles as the whole interval; reading `Регистратор` as
  well fails with an `UnsupportedFeature` diagnostic
