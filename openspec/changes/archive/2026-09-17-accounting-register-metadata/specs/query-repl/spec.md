## ADDED Requirements

### Requirement: Expose accounting-register main-table fields
The main table `РегистрБухгалтерии.<Имя>` SHALL expose `СчетДт`/`СчетКт`
(`AccountDr`/`AccountCr`) for a register with correspondence and `Счет`
(`Account`) without it, every balance dimension and resource under its
own name, every non-balance dimension and resource as `<Имя>Дт`/`<Имя>Кт`
(`<Name>Dr`/`<Name>Cr`), the attributes, and the standard fields `Период`,
`Регистратор`, `НомерСтроки`, `Активность`. The names come from Config
purposes and the physical side suffix of the column, never guessed from
a column's data.

#### Scenario: Debit account
- **WHEN** `ВЫБРАТЬ Т.СчетДт, Т.Сумма, Т.СуммаВалДт ИЗ РегистрБухгалтерии.Управленческий КАК Т`
  is compiled
- **THEN** the SQL reads the debit account column, the balance resource
  column, and the debit column of the non-balance resource

#### Scenario: Non-balance name without a side
- **WHEN** a non-balance resource is read as `Т.СуммаВал`
- **THEN** compilation fails with an `UnknownField` diagnostic

### Requirement: Parse accounting virtual tables with the platform's arity
`РегистрБухгалтерии.<Имя>.Остатки`, `.Обороты`, `.ОстаткиИОбороты`,
`.ОборотыДтКт` and `.ДвиженияССубконто` SHALL accept at most 4, 8, 7, 8
and 5 arguments respectively, and SHALL be reported as an
`UnsupportedFeature` diagnostic naming the table until the stage that
compiles them lands; one argument more SHALL stay a `Syntax` diagnostic.

#### Scenario: Turnovers with an account condition
- **WHEN** `РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет = &Счет, , , , )` is compiled
- **THEN** compilation fails with `UnsupportedFeature`, not with a
  syntax error about the argument count
