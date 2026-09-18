## ADDED Requirements

### Requirement: Compile the extra-dimension values table
`РегистрБухгалтерии.<Имя>.Субконто` (`ExtDimensions`) SHALL compile as
the register's `_AccRgED` table with `Период`, `Регистратор`,
`НомерСтроки`, `УточнениеПериода`, `ВидДвижения` (`Correspond`, the
side of the record), `Вид` (a reference to the chart of characteristic
types) and `Значение` (a value of several types).

#### Scenario: Values of a record
- **WHEN** `ВЫБРАТЬ С.Вид, С.Значение, С.ВидДвижения ИЗ РегистрБухгалтерии.Хозрасчетный.Субконто КАК С ГДЕ С.Регистратор = &Д` is compiled
- **THEN** the SQL reads `_KindRRef`, the `_Value_*` members and
  `_Correspond` of the register's `_AccRgED` table

### Requirement: Extra dimensions of the aggregating tables
`Остатки`, `Обороты` and `ОстаткиИОбороты` of an accounting register
SHALL expose `Субконто<k>` (`ExtDimension<k>`) and `ВидСубконто<k>`
(`ExtDimensionType<k>`) for `k` up to the register's level count, read
from the side's inline columns (`_ValueDt<k>_*`/`_KindDt<k>RRef` on the
debit side, `Ct` on the credit side) under one name, as dimensions
summed away when unread; `Условие` SHALL see them. Without the
`Субконто` argument the positions are the account's own order. With
the argument — one kind or a parenthesized list of kinds, each
`ЗНАЧЕНИЕ(ПланВидовХарактеристик.…)` or a parameter bound to a
reference — `Субконто<j>` SHALL take the value of whichever level
carries the `j`-th listed kind, and records whose account lacks a listed
kind SHALL be excluded.

#### Scenario: Positional extra dimensions
- **WHEN** `ВЫБРАТЬ О.Счет, О.Субконто1, О.СуммаОстаток ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, Счет = &Счет) КАК О` is compiled
- **THEN** each branch reads its side's first value under one name and
  the balance is grouped by account and that value

#### Scenario: Listed kinds
- **WHEN** `Остатки(&Д, , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты))` is read for `Субконто1`
- **THEN** `Субконто1` is the value of the level whose kind is
  `Контрагенты` on the record's side, and records of accounts without
  that kind are excluded

#### Scenario: Unread extra dimensions
- **WHEN** the statement reads `Счет` and a resource only
- **THEN** the extra dimensions are summed away like any dimension
