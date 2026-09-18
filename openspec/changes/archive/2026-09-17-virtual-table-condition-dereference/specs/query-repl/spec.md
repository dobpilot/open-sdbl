## ADDED Requirements

### Requirement: Dereference in virtual-table conditions
A reference path in the condition or the account condition of
`Остатки`, `Обороты` or `ОстаткиИОбороты` of an accumulation or an
accounting register SHALL compile: the target table is joined to the
relation the condition filters with the `LEFT JOIN` and type guard an
ordinary query renders, on the alias of that relation — each side's
branch of a folded accounting table on its own account, the totals and
the movement branch of an accumulation balance on their own base — and
the predicate reads the joined column. A dereference in the access
restriction of such a table SHALL be rendered the same way.

#### Scenario: Account condition through the account's kind
- **WHEN** `РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.Активный))` is compiled
- **THEN** the debit branch joins the chart on `_AccountDtRRef`, the
  credit branch on `_AccountCtRRef`, and both filter on the chart's
  `_Kind`

#### Scenario: Balance filtered through a parent
- **WHEN** `РегистрНакопления.ЗапасыНаСкладах.Остатки(&Д, Номенклатура.Родитель = &Р)` is compiled
- **THEN** both the totals branch and the movement branch join the
  catalog on their own alias
