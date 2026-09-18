## 0. Corpus and measurements

- [x] 0.1 Dump the `unf` base with `tools/corpus/fetch_base.py`, select
  the coverage subset, prune it with `cargo test --test corpus_fixture --
  --ignored`, bind the date parameters, record the corpus, and write
  `tests/fixtures/unf/README.md` (counts per virtual table, refusals by
  cause).
- [ ] 0.2 Measure on the platform (probe configuration with one accounting
  register with correspondence, one without, a chart of accounts with two
  extra dimensions, one balance and one non-balance dimension and
  resource): the physical layout of `_AccRg`, `_AccRgED`, `_AccRgAT*`,
  `_AccRgCT`, `_Acc*` tables; the Config collection GUIDs and flag
  positions; the SQL the platform sends for each virtual table
  (`log_statement='all'`); the answers to the probe queries listed in
  `design.md`.
- [ ] 0.3 Settle the open questions of `design.md` with the maintainer and
  fold the answers into the requirement deltas.
- [x] 0.4 Close the accumulation-register gaps the corpus found, as a
  change each: system enumerations in `ЗНАЧЕНИЕ`,
  `<Ресурс>Приход`/`Расход` on `Обороты`, the `Авто` and `Период`
  periodicities, `ПЕРВЫЕ 0`, the tuple `В (ВЫБРАТЬ …)` condition, the
  `Узел` field of `Изменения`, `ИЗ &Таблица` as `UnsupportedFeature`
  (archived 2026-09-17; 205 → 320 compiling UNF queries).
- [x] 0.2a Measured on `unf` (see `design.md`): the `_AccRg`, `_AccRgAT0`,
  `_AccRgCT`, `_Acc` layouts, the account kinds, no extra dimensions in
  UNF, predefined accounts in the `.9` resource.

## 1. Metadata

- [x] 1.1a Accounting-register field purposes with balance flags and the
  register's chart of accounts (archived `accounting-register-metadata`,
  2026-09-17). Correspondence is told by the main table's columns.
- [x] 1.1b Predefined accounts from the `.9` resource: the decoder, the
  acquisition queries of both providers, the corpus tools,
  `ЗНАЧЕНИЕ(ПланСчетов.X.Счет)` (archived `chart-of-accounts-predefined-items`).
  Charts of characteristic types (`.7`) and of calculation types (`.3`)
  wait for the probe base: those suffixes are shared with other classes.
- [ ] 1.1c The chart's extra-dimension count and per-account
  extra-dimension kinds (needs the probe base: UNF has none).
- [x] 1.2 Tests on the UNF fixture (`tests/query_accounting.rs`) and the
  measured descriptor (`config.rs`).

## 2. Main table and extra dimensions

- [x] 2.1 `СчетДт`/`СчетКт`/`Счет`, `<Измерение>Дт/Кт`, `<Ресурс>Дт/Кт`,
  `Активность` (archived `accounting-register-metadata`); `УточнениеПериода`
  is simply not exposed.
- [ ] 2.2 `РегистрБухгалтерии.X.Субконто` as a queryable service table
  with `Вид`, `ВидДвижения`, `Значение`, `Период`, `Регистратор`,
  `НомерСтроки`.
- [x] 2.3 Lexer keywords and parser slots for the five virtual tables with
  their argument counts (4, 8, 7, 8, 5); each stops on a staged
  `UnsupportedFeature` diagnostic.

## 3. Virtual tables, one change each

- [x] 3.1 `ДвиженияССубконто(Начало, Конец, Условие, Порядок, Первые)`
  without `Порядок`/`Первые` (archived `accounting-record-level-tables`).
- [x] 3.2 `Обороты(Начало, Конец, Периодичность, УсловиеСчета, Субконто,
  Условие, УсловиеКорСчета, КорСубконто)` without the `Субконто` and
  `Кор…` arguments (archived `accounting-turnovers`); the platform omits
  the `Субконто` arguments for a chart without extra dimensions.
- [x] 3.3 `ОборотыДтКт(Начало, Конец, Периодичность, УсловиеСчетаДт,
  СубконтоДт, УсловиеСчетаКт, СубконтоКт, Условие)` (archived
  `accounting-record-level-tables`; the buh corpus executes it).
- [x] 3.4 `Остатки(Период, УсловиеСчета, Субконто, Условие)` without
  `Субконто` and without `РазвернутыйОстаток` (archived
  `accounting-balances`).
- [x] 3.5 `ОстаткиИОбороты(Начало, Конец, Периодичность,
  МетодДополненияПериодов, УсловиеСчета, Субконто, Условие)` without
  `Субконто` (archived `accounting-balances`).
- [x] 3.6a Account conditions through the account's attributes
  (`Счет.ТипСчета = …`), for both register kinds (archived
  `virtual-table-condition-dereference`).
- [x] 3.6b The extra-dimension list argument, positional and explicit
  (archived `accounting-extra-dimensions`, measured on buh).
- [ ] 3.6c `Счет В ИЕРАРХИИ (…)` in the account condition.
- [x] 3.7a `КорСчет`/`КорСубконто`/`<Измерение>Кор` of `Обороты` with
  `УсловиеКорСчета` (archived `accounting-balanced-account`, reconciled
  with the records on the live buh base).
- [x] 3.7b `Порядок`/`Первые` of `ДвиженияССубконто` (archived
  `records-order-and-top`; `УБЫВ` inside the argument is not parsed).
- [x] 3.7c `РазвернутыйОстаток` fields of `Остатки` and the whole-interval
  `ОстаткиИОбороты` (archived `accounting-expanded-balances`).
- [ ] 3.7d A register without correspondence — no corpus query reads it.

## 4. Verification and documentation

- [ ] 4.1 Goldens on both dialects; platform probes for every table on the
  probe base; the UNF corpus rerecorded after every stage.
- [ ] 4.2 README and `docs/query-language-support.md` rows 190–195; the
  five CI checks and strict OpenSpec validation per commit.
