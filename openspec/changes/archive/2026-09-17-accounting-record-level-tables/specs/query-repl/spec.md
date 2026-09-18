## ADDED Requirements

### Requirement: Compile debit-credit turnovers
`РегистрБухгалтерии.<Имя>.ОборотыДтКт(Начало, Конец, Периодичность,
УсловиеСчетаДт, СубконтоДт, УсловиеСчетаКт, СубконтоКт, Условие)` — the
`Субконто` arguments absent for a register without extra dimensions —
SHALL answer one row per `СчетДт`/`СчетКт` pair, the balance
dimensions, the `Дт`/`Кт` sides of the non-balance dimensions,
`СубконтоДт<k>`/`ВидСубконтоДт<k>`/`СубконтоКт<k>`/`ВидСубконтоКт<k>` in
use and the split of the periodicity, over the active records of
`[Начало, Конец)`, with `<Ресурс>Оборот` per balance resource and
`<Ресурс>ОборотДт`/`<Ресурс>ОборотКт` per non-balance one, unread
dimensions summed away. A listed kind of one side SHALL map that side's
`Субконто<j>` and exclude the records whose account on that side lacks
it. The conditions SHALL see the record's own fields.

#### Scenario: Correspondence with extra dimensions
- **WHEN** `ВЫБРАТЬ О.СчетДт, О.СчетКт, О.СубконтоДт1, О.СуммаОборот ИЗ РегистрБухгалтерии.Хозрасчетный.ОборотыДтКт(&Н, &К, , СчетДт В (&Счета), , , , Организация = &Орг) КАК О`
  is compiled
- **THEN** the SQL groups the main table by both accounts and the debit
  side's first value and sums the balance resource

#### Scenario: Without extra dimensions
- **WHEN** the UNF register is read with six arguments
- **THEN** it compiles, and an eighth argument is a `Syntax` diagnostic

### Requirement: Compile the records with extra dimensions
`РегистрБухгалтерии.<Имя>.ДвиженияССубконто(Начало, Конец, Условие,
Порядок, Первые)` SHALL answer the records of `[Начало, Конец)` with
the main table's fields and `СубконтоДт<k>`, `ВидСубконтоДт<k>`,
`СубконтоКт<k>`, `ВидСубконтоКт<k>` read from the inline columns;
`Условие` SHALL see the same fields; `Порядок` and `Первые` SHALL be
`UnsupportedFeature` diagnostics.

#### Scenario: Records of one contractor
- **WHEN** `ВЫБРАТЬ Д.Регистратор, Д.СубконтоКт1, Д.Сумма ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, СубконтоКт1 = &Контрагент) КАК Д`
  is compiled
- **THEN** the SQL reads the register's rows with the period bounds and
  the credit side's first value compared
