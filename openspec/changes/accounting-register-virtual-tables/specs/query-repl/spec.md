## ADDED Requirements

### Requirement: Compile the extra-dimensions table
`РегистрБухгалтерии.<Имя>.Субконто` SHALL compile as the extra-dimension
table with `Период`, `Регистратор`, `НомерСтроки`, `ВидДвижения`, `Вид`
and `Значение`; `Значение` is a composite value with the usual dereference
behaviour, `Вид` a reference to the chart of characteristic types of the
chart of accounts, and `ВидДвижения` the debit/credit side of the record.

#### Scenario: Join to the records
- **WHEN** the table is joined to the main table by `Регистратор` and
  `НомерСтроки`
- **THEN** each record answers one row per extra-dimension value it has

### Requirement: Compile the records-with-extra-dimensions table
`РегистрБухгалтерии.<Имя>.ДвиженияССубконто(Начало, Конец, Условие,
Порядок, Первые)` SHALL answer the records of `[Начало, Конец]` with
`СубконтоДт<N>`, `ВидСубконтоДт<N>`, `СубконтоКт<N>`, `ВидСубконтоКт<N>`
(`Субконто<N>`, `ВидСубконто<N>` without correspondence) for `N` up to
the chart's extra-dimension count, pivoted from the extra-dimension table
by position. `Условие` MAY name the record fields and the extra
dimensions; `Порядок` and `Первые` SHALL order and cut the records before
the pivot.

#### Scenario: Two extra dimensions
- **WHEN** the chart allows two extra dimensions and the query reads
  `СубконтоДт1` and `СубконтоДт2`
- **THEN** each row carries the first and second debit extra dimension of
  its record, `NULL` where the account has fewer

### Requirement: Compile accounting turnovers
`РегистрБухгалтерии.<Имя>.Обороты(Начало, Конец, Периодичность,
УсловиеСчета, Субконто, Условие, УсловиеКорСчета, КорСубконто)` SHALL
answer, per account, the dimensions in use and the extra dimensions in
use, `<Ресурс>Оборот`, `<Ресурс>ОборотДт`, `<Ресурс>ОборотКт` (and the
`Кор…` counterparts of a register with correspondence), summed over the
fields the statement never reads like every register table. The account
condition SHALL be a predicate on the `Счет` field, including `В
ИЕРАРХИИ`; the extra-dimension list SHALL be positional by the account's
extra-dimension kinds when omitted and explicit by kind when given; a
periodicity SHALL follow the accumulation-register rules. What a stage
does not support SHALL be an `UnsupportedFeature` diagnostic that names
the argument.

#### Scenario: Turnovers by account and first extra dimension
- **WHEN** `ВЫБРАТЬ О.Счет, О.Субконто1, О.СуммаОборотДт ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет В ИЕРАРХИИ (&Счет)) КАК О`
  is executed
- **THEN** there is one row per account under `&Счет` and per first extra
  dimension with the debit turnover of the interval

### Requirement: Compile debit-credit turnovers
`РегистрБухгалтерии.<Имя>.ОборотыДтКт(Начало, Конец, Периодичность,
УсловиеСчетаДт, СубконтоДт, УсловиеСчетаКт, СубконтоКт, Условие)` SHALL
answer turnovers per pair of `СчетДт`/`СчетКт`, the debit and credit
dimensions and extra dimensions in use, with `<Ресурс>Оборот` and the
`Дт`/`Кт` variants of non-balance resources, summed over the unread
fields.

#### Scenario: Correspondence of two accounts
- **WHEN** the table is read with `СчетДт = &Дт` and `СчетКт = &Кт`
- **THEN** one row holds the turnover between the two accounts over the
  interval

### Requirement: Compile accounting balances
`РегистрБухгалтерии.<Имя>.Остатки(Период, УсловиеСчета, Субконто,
Условие)` SHALL answer, per account and per dimensions and extra
dimensions in use, `<Ресурс>Остаток` (debit minus credit),
`<Ресурс>ОстатокДт` and `<Ресурс>ОстатокКт` (the positive and the negated
negative balance) and `<Ресурс>РазвернутыйОстатокДт`/`Кт` (the sums of
the debit and credit balances of the finer combinations), computed from
the movements strictly before `Период`, and SHALL drop combinations whose
every balance is zero. `ОстаткиИОбороты` SHALL extend it with the
opening and closing balances and the turnovers of `[Начало, Конец]` under
the accumulation-register rules for periodicity and completion method.

#### Scenario: Balance at a point
- **WHEN** `Остатки(&Дата, Счет = &Счет)` is read
- **THEN** the balance holds every active record before `&Дата`, debits
  counted positive and credits negative

#### Scenario: Zero balance
- **WHEN** every resource balance of one combination is zero
- **THEN** the combination is absent from the result
