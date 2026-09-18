## ADDED Requirements

### Requirement: Compile accounting turnovers
`РегистрБухгалтерии.<Имя>.Обороты(Начало, Конец, Периодичность,
УсловиеСчета, Субконто, Условие, УсловиеКорСчета, КорСубконто)` of a
register with correspondence — or `Обороты(Начало, Конец, Периодичность,
УсловиеСчета, Условие, УсловиеКорСчета)` when the register keeps no
extra dimensions, the platform omitting the `Субконто` arguments then
(measured on the UNF configuration) — SHALL answer, per account (`Счет`), the
dimensions in use and the calendar period when a periodicity is given,
`<Ресурс>Оборот` (debit minus credit), `<Ресурс>ОборотДт` and
`<Ресурс>ОборотКт`, computed from the active records of `[Начало,
Конец)` folded into a debit row and a credit row each; a non-balance
dimension or resource SHALL be read from the side's own column under its
side-less name. Unread dimensions SHALL be summed away like every
register table. `УсловиеСчета` SHALL be a predicate on `Счет`, the
side's account; `Условие` SHALL be a predicate on the side's view of the
record. The extra-dimension list, the balanced-account arguments, the
`Авто` periodicity and a register without correspondence SHALL be
`UnsupportedFeature` diagnostics naming what is missing; `Регистратор`
and `Запись` SHALL split the rows like the accumulation table does.

#### Scenario: Turnovers by account and organization
- **WHEN** `ВЫБРАТЬ О.Счет, О.Организация, О.СуммаОборотДт ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет = &Счет) КАК О`
  is compiled
- **THEN** the SQL unions a debit branch and a credit branch of the main
  table, applies the account condition to each side's account, and sums
  the debit rows into `СуммаОборотДт`

#### Scenario: Non-balance resource
- **WHEN** `О.СуммаВалОборотКт` is read
- **THEN** the credit branch reads `_Fld<N>Ct` and the debit branch
  `_Fld<N>Dt` under one column, and the credit sum answers the column

#### Scenario: Condition fifth without extra dimensions
- **WHEN** `Обороты(&Н, &К, МЕСЯЦ, , СценарийПланирования = &С)` is compiled for the UNF register
- **THEN** the fifth argument is the condition and a seventh argument is
  a `Syntax` diagnostic

#### Scenario: Balanced account requested
- **WHEN** the balanced-account condition is given
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
