## Context

The compiler handles accumulation registers completely: field purposes
from Config, `Остатки` on the totals anchor plus movement delta,
`Обороты` with periodicity, `ОстаткиИОбороты`, and the pruning of unread
dimensions (`codegen/virtual_tables.rs`). Accounting registers reach only
`РегистрБухгалтерии.<Имя>` as a plain table: `ConfigFieldPurpose` has no
accounting variants, the chart of accounts is a plain reference target,
and the extra-dimension table is invisible. The six virtual tables are
rows 190–195 of `docs/query-language-support.md`, all ❌.

The UNF configuration keeps a management accounting register with
correspondence over the chart `Управленческий`. Its corpus
(`tests/fixtures/unf`, 1633 register queries in the configuration, 523
kept) reads the accounting register in 37 queries: `ОстаткиИОбороты` 20,
the main table 7 (`СчетДт`/`СчетКт`, `Активность`), `Обороты` 6,
`Остатки` 4, `Субконто` 1 (against `Хозрасчетный`, a register UNF does
not have — dead code for another configuration). Nobody reads
`ОборотыДтКт` or `ДвиженияССубконто`. That fixes the order of the stages
below: metadata and main-table fields, then `Обороты`, `Остатки` and
`ОстаткиИОбороты` on top of the same relation, and the two record-level
tables last.

The same corpus stopped on accumulation-register gaps that were cheaper
and went first (task 0.4, done: 205 → 320 compiling queries): system enumerations in
`ЗНАЧЕНИЕ(ВидДвиженияНакопления.Приход)` (also `ВидДвиженияБухгалтерии`
and `ВидСчета`, which the accounting tables need anyway),
`<Ресурс>Приход`/`<Ресурс>Расход` on `Обороты` of a balance register,
the `Авто` and `Период` periodicities, `ПЕРВЫЕ 0`, a tuple
`(А, Б) В (ВЫБРАТЬ …)` in a virtual-table condition, the `Узел` field of
`Изменения`, and `ИЗ &Таблица` reported as `UnsupportedFeature` instead
of a syntax error. What the corpus still stops on, outside accounting:
temporary tables of earlier batches (53), the balance columns of a split
`ОстаткиИОбороты` (27 — running sums, a window-function change for
PostgreSQL and SQL Server 2012 that 2008 cannot follow), value-table
parameters (14), dereferences inside virtual-table conditions (11).

## What must be measured before coding

Everything below is what the platform documentation and the known storage
layout say; each item is confirmed on the probe base (see the probe
method in the project memory: `ibcmd` + `ibsrv`, `log_statement='all'`)
before the corresponding stage starts.

### Measured on `unf` (2026-09-17)

- The register `Управленческий` has **no extra dimensions**: the chart
  `_Acc17` has an empty `_Acc17_VT407` (extra-dimension kinds) and no
  `_AccRgED` table exists. Stages that pivot extra dimensions cannot be
  measured on UNF; the probe base carries them.
- `_AccRg411`: `_Period`, `_RecorderTRef/_RecorderRRef`, `_LineNo`,
  `_Active`, `_AccountDtRRef`, `_AccountCtRRef`, balance dimensions
  `_Fld412RRef`, `_Fld413RRef`, a non-balance dimension
  `_Fld414DtRRef/_Fld414CtRRef`, balance resources `_Fld415`,
  `_Fld31842`, non-balance resources `_Fld416Dt/_Fld416Ct`,
  `_Fld31843Dt/_Fld31843Ct`, the attribute `_Fld417`, the common
  attribute `_Fld405`. No `_EDHash*` columns without extra dimensions.
- `_AccRgAT0418` (totals by account): `_AccountRRef`, `_Period`, the
  dimensions (a non-balance one as a single `_Fld414RRef`), `_Fld415` (the
  balance), `_TurnoverDt419`, `_TurnoverCt420`, `_Turnover421` per
  resource (the non-balance resource keeps `_Fld416` + turnovers too).
- `_AccRgCT425` (correspondence turnovers): `_Period`, `_AccountDtRRef`,
  `_AccountCtRRef`, the dimensions with `Dt/Ct` for the non-balance one,
  the resources with `Dt/Ct` for the non-balance ones. `_AccRgOpt426`
  and `_AccRgChngR14553` exist.
- `_Acc17`: `_Kind` (0 active, 1 passive, 2 active/passive — confirmed
  against the predefined accounts), `_OffBalance`, `_OrderField`,
  `_ParentIDRRef`, `_PredefinedID`, three attributes.
- **Predefined accounts** are not in a `<guid>.1c` resource: the chart
  keeps them in `<guid>.9` (a value table `{2, {1, {10, …}}, …}` whose
  rows carry the item's reference type and GUID, the name, the code, the
  description, the kind and the off-balance flag). Charts of
  characteristic types have `.7`, charts of calculation types `.3` —
  to be decoded the same way. Until then `ЗНАЧЕНИЕ(ПланСчетов.X.Счет)`
  cannot resolve, and the CONFIG acquisition query fetches `.1c` only.
  This goes into stage 1 (metadata).

- **Argument layout without extra dimensions.** UNF writes
  `Обороты(&Н, &К, МЕСЯЦ, , СценарийПланирования = …)` and
  `ОстаткиИОбороты(&Н, &К, Авто, , Счет.ТипСчета = …, СценарийПланирования = …)`:
  the platform omits the `Субконто` arguments of every virtual table when
  the chart of accounts has no extra dimensions, so the condition moves
  up one slot. The compiler keys the layout on the register's `AccRgED`
  table.

### Measured on `buh` (demo Бухгалтерия предприятия, 2026-09-17)

The register `Хозрасчетный` over the chart `_Acc69` with three extra
dimensions replaces the probe base for everything below:

- `_AccRg38456` carries the extra-dimension values **inline** beside
  `_AccountDtRRef`/`_AccountCtRRef`: per level `k` in 1..=3 and per side
  `_ValueDt<k>_TYPE/_RTRef/_RRRef`, `_KindDt<k>RRef` and the `Ct`
  counterparts, then `_EDHashDt`, `_EDHashCt`; also `_PeriodAdjustment`
  (`УточнениеПериода`) after `_Period`. Balance dimension `_Fld38457RRef`,
  non-balance `_Fld38458Dt/Ct`, `_Fld38459Dt/Ct`, balance resource
  `_Fld38460`, non-balance `_Fld38461…38465Dt/Ct`, attributes `_Fld38466`,
  `_Fld38467`, separator `_Fld2331`.
- `_AccRgED38493` (146 322 rows for 30 836 records): `_Period`,
  `_PeriodAdjustment`, `_Recorder*`, `_LineNo`, `_Correspond` (0 —
  65 884 rows, 1 — 80 438), `_KindRRef`, `_Value_TYPE/_RTRef/_RRRef`,
  `_Fld2331`. One row per record, side and level.
- Totals: `_AccRgAT0` (by account), `_AccRgAT1…3` with `_Value<k>_*` per
  level and `_TurnoverDt/_TurnoverCt/_Turnover` per resource,
  `_AccRgCT` (correspondence: `_AccountDt/Ct`, dimensions per side).
- `_Acc69_ExtDim38448` (the chart's `ВидыСубконто`): `_Acc69_IDRRef`,
  `_LineNo` (the position), `_DimKindRRef`, `_DimIsMetadata`,
  `_TurnoverOnly`; 799 rows, at most three per account (`62` has three,
  the third turnover-only).
- The corpus (`tests/fixtures/buh`, 102 accounting queries) stops on the
  `Субконто` argument and fields (31), `ОборотыДтКт` (10),
  `ДвиженияССубконто` (7), the `Субконто` table read as a tabular section
  (8), the balanced-account arguments (4).

### Physical layout to confirm on the probe base

- `_AccRg<N>`: `_Period`, `_RecorderTRef`/`_RecorderRRef`, `_LineNo`,
  `_Active`, `_AccountDtRRef`/`_AccountCtRRef` (with correspondence) or
  `_AccountRRef`, balance dimensions `_Fld<K>RRef`, non-balance dimensions
  `_Fld<K>DtRRef`/`_Fld<K>CtRRef`, balance resources `_Fld<K>`, non-balance
  resources `_Fld<K>Dt`/`_Fld<K>Ct`, attributes, `_EDHashDt`/`_EDHashCt`
  (a hash of the extra-dimension set per side).
- `_AccRgED<N>`: `_Period`, `_Recorder*`, `_LineNo`, `_Correspond`
  (0 debit, 1 credit), `_KindRRef` (chart of characteristic types),
  `_Value_TYPE`/`_Value_RTRef`/`_Value_RRRef`/… composite, plus the
  turnover resources copied per row (to confirm).
- Totals: `_AccRgAT0<N>` (per account, period, dimensions, `_Splitter`),
  `_AccRgAT1..k<N>` (per account and `k` extra dimensions with
  `_Kind<i>RRef` and `_Value<i>_*`), `_AccRgCT<N>` (correspondence
  turnovers per `_AccountDtRRef`/`_AccountCtRRef` and dimensions),
  `_AccRgOpt<N>` (totals options: `_MinPeriod`, `_ActualPeriod`,
  `_UseTotals`, `_UseSplitter`).
- Chart of accounts `_Acc<N>`: `_Kind` (0 active, 1 passive, 2
  active/passive), `_OffBalance`, `_Order`, `_ParentIDRRef`,
  `_Code`, `_Description`; `_Acc<N>_ExtDim<M>` (inline table): `_LineNo`,
  `_DimKindRRef`, `_TurnoversOnly`; the DBNames aliases of every table.

### Config (from the `unf` descriptors)

- Collection GUIDs of the accounting-register dimensions, resources and
  attributes, and the position of the `Балансовый` flag on a dimension and
  a resource (`ConfigFieldPurpose::AccountingRegisterDimension { balance }`
  or a separate flag field).
- Register properties: `ПланСчетов` (a reference type GUID), `Корреспонденция`,
  `ПериодичностьИтогов`? (not needed by SQL).
- Chart of accounts: `МаксКоличествоСубконто`, `ВидыСубконто` (the chart of
  characteristic types), `ПризнакиУчета`/`ПризнакиУчетаСубконто` (only if a
  corpus query reads them).

### Platform SQL and answers (probe base)

For each virtual table, the SQL the platform sends and the rows it
answers on a small data set with: one balance and one non-balance
dimension, one balance and one non-balance resource, two accounts with
two extra dimensions in opposite order, an off-balance account, a record
with `Активность = ЛОЖЬ`. Specific questions the SQL answers:

1. Does `Обороты` read `_AccRgCT`/`_AccRgAT` totals or the movements with
   `_AccRgED` joined? (Accumulation `Обороты` here reads movements; the
   totals anchor is used for `Остатки` only.)
2. `РазвернутыйОстатокДт/Кт`: over which finer combination the split is
   summed when the query groups by account only (all extra dimensions
   of the account, or the dimensions only).
3. `Остаток` vs `ОстатокДт`/`ОстатокКт` for an active/passive account
   with a negative balance; whether `_Kind` influences anything in SQL
   (expected: no, the sign alone decides).
4. Positional extra dimensions without the `Субконто` argument: the pivot
   by `_Acc_ExtDim._LineNo` per account; with the argument: the filter
   to accounts that carry every listed kind and the pivot by kind.
5. Account condition: `Счет В ИЕРАРХИИ (&Счет)` becomes the parent chain
   of `_Acc` (the hierarchy CTE the compiler already renders) — confirm
   the platform includes the account itself.
6. `ДвиженияССубконто`: the `Первые`/`Порядок` arguments applied before
   the extra-dimension pivot; whether `Конец` is inclusive (the `Период`
   arguments of `Обороты` are `[begin, end]` inclusive on the platform
   for accounting tables? — measure, the accumulation tables here are
   half-open by spec).
7. Both period bounds accept the same expressions as the accumulation
   tables (`ДАТАВРЕМЯ`, parameter, `НАЧАЛОПЕРИОДА`/`КОНЕЦПЕРИОДА`/
   `ДОБАВИТЬКДАТЕ`).

## Decisions

- **Movements first, totals later.** Every aggregating table is rendered
  from `_AccRg` (+ `_AccRgED` when an extra dimension is read or
  filtered), like accumulation `Обороты` today. The `_AccRgAT*` anchor for
  `Остатки` is a later optimization with its own change; correctness does
  not depend on it, and the totals tables are absent when totals are
  switched off for the register.
- **One relation per side.** With correspondence, a debit-side read and a
  credit-side read of the same record are two rows of a `UNION ALL`
  (`СчетДт` as `Счет` with `+resource`, `СчетКт` as `Счет` with
  `-resource`), which is how `Обороты`/`Остатки` fold the two accounts of
  one record into the account dimension. `ОборотыДтКт` keeps the record
  as one row.
- **Extra dimensions by pivot.** `Субконто<N>` is a `LEFT JOIN` of
  `_AccRgED` per position (`_Correspond` = side, `_LineNo` of the kind in
  `_Acc_ExtDim` for the positional form, `_KindRRef` = the listed kind for
  the explicit form). The value is the composite `_Value_*` triple, which
  the expression compiler already dereferences and compares.
- **Pruning as for accumulation tables.** Account, dimensions and extra
  dimensions the statement never reads are summed away by the existing
  `__aggregate_used` machinery; the account is a dimension of these
  tables for that purpose.
- **Diagnostics name the argument.** Each stage refuses what it does not
  implement with `UnsupportedFeature` and the argument name
  (`Периодичность`, `КорСубконто`, `МетодДополненияПериодов`), so a
  corpus query stops at a readable place.
- **Sealed backend stays sealed.** Both dialects render the same shape;
  SQL Server 2008 constraints (no window functions in
  `ОстаткиИОбороты` per period) carry over from the accumulation tables.

## Staging (one OpenSpec change and one commit per stage)

0. Corpus (done: `tests/fixtures/unf`), the accumulation gaps the corpus
   found (task 0.4, done), probes on the platform (task 0.2), answers to
   the open questions (task 0.3, done 2026-09-17).
   Stage 1 is split off as the change `accounting-register-metadata`:
   purposes, balance flag, chart link, main-table names, arity.
1. Metadata: purposes, flags, chart links; predefined items of charts
   (`.9`/`.7`/`.3` resources, the acquisition query, the corpus tooling)
   so `ЗНАЧЕНИЕ(ПланСчетов.X.Счет)` resolves; the main table's `Дт`/`Кт`
   fields; the `Субконто` service table; lexer keywords and parser slots
   with the platform's argument counts, so the corpus stops on semantics,
   not on arity. (`ЗНАЧЕНИЕ(ВидДвиженияБухгалтерии.…)` and `Счет.Вид`
   are done.)
2. `Обороты` by account and dimensions without extra dimensions,
   periodicity or `Кор…` (UNF reads it this way: `Счет`, `Организация`,
   `СуммаОборотДт/Кт`); then the extra-dimension pivot (`Субконто<N>`)
   shared by every later table; then periodicity by the accumulation
   rules; then `КорСчет`/`КорСубконто`.
3. `Остатки` from movements on the same relation; `ОстаткиИОбороты` on
   top of the balance and turnover relations (the most-read table in
   UNF: 20 queries, mostly `СуммаНачальныйОстаток`/`СуммаКонечныйОстаток`
   by `Счет` with an account condition on `Счет.ТипСчета`).
4. Account condition on the hierarchy and through the account's
   attributes (`Счет.ТипСчета = ЗНАЧЕНИЕ(…)`, `Счет.Вид`), the explicit
   extra-dimension list.
5. `ОборотыДтКт` and `ДвиженияССубконто` (unused by UNF, needed for
   completeness).
6. The totals anchor for `Остатки` (optional).

## Status on 2026-09-17

Stages 1–3 and the account condition of stage 4 are archived
(`accounting-register-metadata`, `chart-of-accounts-predefined-items`,
`accounting-turnovers`, `accounting-balances`,
`virtual-table-condition-dereference`): 24 of the 36 accounting queries of
the UNF corpus compile, the SQL of every table was executed on the live
UNF base. What the rest stops on: the balance columns of a split
`ОстаткиИОбороты` (3, running balances), `КорСчет` (2 + 1), joins
written inside a join's source (2), `РазвернутыйОстаток` (2), a
completion method without a periodicity (1). Everything that needs
extra dimensions waits for the probe base.

Later the same day the balances of a split `ОстаткиИОбороты` became
running sums (archived `periodic-running-balances`): 384 of 517 UNF
queries compile, 27 of the 36 accounting ones.

## Open questions for the maintainer

1. **Order of stages.** Recommended: as staged above, by corpus count
   (`ОстаткиИОбороты` 20, main 7, `Обороты` 6, `Остатки` 4, the rest 0).
2. **Registers without correspondence.** Recommended: support both
   layouts from stage 1 (the field set differs, the codegen is a subset),
   even though UNF has only the correspondence one — the probe base
   carries one of each.
3. **Totals anchor.** Recommended: leave `_AccRgAT*` out until the
   movement-based tables are measured correct; the accumulation anchor
   showed the delta logic is the hard part.
4. **`РазвернутыйОстаток`.** Recommended: implement only after the probe
   shows the grain; refuse the columns until then.
5. **`ПериодГод`…`ПериодСекунда` fields.** They are refused for
   accumulation tables too; keep refusing (consistent), or add them for
   both kinds in a separate change.
6. **Extra-dimension list argument.** Recommended: accept a list of
   `ПланВидовХарактеристик` predefined names or references in
   parentheses and a parameter bound to a reference; refuse a parameter
   bound to an array (the compiler has no array parameters).
7. **Inclusive `Конец`.** If the probe shows an inclusive end bound for
   accounting tables while accumulation tables here are half-open, the
   accounting tables follow the platform; the accumulation discrepancy is
   already flagged in the project memory.
8. **Version.** Recommended: 0.4.0 for the stage that ships `Обороты`
   and `Остатки`, since the language table gains a whole register kind.

## Out of scope

`РегистрРасчета` virtual tables; `УточнениеПериода`; accounting flags
(`ПризнакУчета`) unless the corpus reads them; the `ВидыСубконто` service
table of the chart is already queryable and stays as it is.
