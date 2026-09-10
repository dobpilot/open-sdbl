## MODIFIED Requirements

### Requirement: Compile a bounded read-only 1C query subset
The `open-sdbl` library SHALL compile one or more
`ВЫБРАТЬ`/`SELECT` branches into one PostgreSQL SELECT statement using only
authoritative resolved metadata. Branches MAY be connected with
`ОБЪЕДИНИТЬ`/`UNION` or `ОБЪЕДИНИТЬ ВСЕ`/`UNION ALL`. An unjoined
branch SHALL support projection or `*`, one metadata source, an optional source
alias with or without `КАК`/`AS`, one-hop reference property paths,
`РАЗЛИЧНЫЕ`/`DISTINCT`, `ПЕРВЫЕ`/`TOP`, and basic `ГДЕ`/`WHERE`
expressions. A branch MAY instead chain one or more
`[ВНУТРЕННЕЕ] СОЕДИНЕНИЕ` / `[INNER] JOIN`, `ЛЕВОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` / `LEFT [OUTER] JOIN`, and `ПРАВОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` /
`RIGHT [OUTER] JOIN` in source order, each introducing one more source
scope, or contain exactly one `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` / `FULL
[OUTER] JOIN`. Joined branches SHALL support named direct fields and one-hop
reference properties. Each join condition SHALL contain at least one top-level
scalar direct-field equality between the joined source and an earlier source
and MAY combine that anchor with additional supported scalar direct-field
predicates over the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. Final `УПОРЯДОЧИТЬ
ПО`/`ORDER BY` SHALL support `ВОЗР`/`ASC` and `УБЫВ`/`DESC`. Statements
of a batch SHALL be separated by semicolons and one or more trailing
semicolons SHALL terminate the batch; a single statement remains a valid
batch. Unsupported syntax SHALL fail before execution.

#### Scenario: Logical catalog query
- **WHEN** a query selects `Код` and a custom attribute from
  `Справочник.<name>`
- **THEN** generated SQL uses the DBNames-resolved table and Config-resolved
  physical columns without inferring a numeric name

#### Scenario: Reference property projection
- **WHEN** a query selects `Организация.Код` from a source with a fixed
  `Организация` reference
- **THEN** generated SQL left-joins the SchemaStorage-declared target through
  its ID and projects the target Code column

#### Scenario: Reused reference join
- **WHEN** the same reference path is used in projection, filtering, or ordering
- **THEN** the generated SQL contains one shared join for that source reference

#### Scenario: Implicit source alias and reference property
- **WHEN** a query selects `t.Регистратор.Номер` from a source followed
  directly by alias `t`
- **THEN** the alias qualifies the source and generated SQL left-joins the
  SchemaStorage-declared recorder target to project its Number column

#### Scenario: Explicit source alias
- **WHEN** a source alias follows `КАК` or `AS`
- **THEN** it has the same qualification and SQL-generation semantics as an
  implicit alias

#### Scenario: Clause after an unaliased source
- **WHEN** `ГДЕ`/`WHERE` or `УПОРЯДОЧИТЬ`/`ORDER` immediately follows the source
- **THEN** the clause keyword is not consumed as an implicit alias

#### Scenario: UNION duplicate elimination
- **WHEN** two compatible branches are connected with `ОБЪЕДИНИТЬ` or `UNION`
- **THEN** each branch is compiled independently and PostgreSQL removes
  duplicate combined rows

#### Scenario: UNION ALL duplicate preservation
- **WHEN** compatible branches are connected with `ОБЪЕДИНИТЬ ВСЕ` or
  `UNION ALL`
- **THEN** generated SQL retains duplicate rows

#### Scenario: Union result shape and ordering
- **WHEN** compatible branches have equal logical and expanded SQL projection
  widths followed by final ordering
- **THEN** result labels come from the first branch and ordering addresses the
  combined output rather than a branch table alias

#### Scenario: Incompatible union branches
- **WHEN** a later branch has a different logical or expanded SQL projection
  width
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: INNER JOIN
- **WHEN** two metadata sources use `СОЕДИНЕНИЕ`/`JOIN` or its explicit
  `ВНУТРЕННЕЕ`/`INNER` form with a supported condition
- **THEN** generated PostgreSQL uses INNER JOIN and returns matching pairs only

#### Scenario: LEFT JOIN with a reference projection
- **WHEN** a query selects `Регистратор.Номер` and a right-source field
  through a supported `LEFT JOIN`
- **THEN** generated PostgreSQL preserves every left row, uses the main LEFT
  JOIN, and independently resolves the recorder reference property

#### Scenario: RIGHT JOIN
- **WHEN** two metadata sources use `ПРАВОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` or
  `RIGHT [OUTER] JOIN`
- **THEN** generated PostgreSQL preserves every right row with NULL values for
  an absent left side

#### Scenario: Chained joins
- **WHEN** a branch joins a document, its tabular section, and a catalog with
  three sources in a row, the last condition referencing the first source
- **THEN** generated SQL emits the joins in source order with their own ON
  conditions, followed by any reference-property joins, and every source is
  addressable by its alias

#### Scenario: Same object under two aliases
- **WHEN** a branch joins `Справочник.Контрагенты` twice under different
  aliases
- **THEN** each alias resolves to its own scope and generated SQL contains two
  joins on that table

#### Scenario: FULL JOIN matched and unmatched rows
- **WHEN** two aliased metadata sources are connected by a supported FULL JOIN
- **THEN** the result contains all matching combinations and every unmatched
  row from both sources with NULL values for the absent side

#### Scenario: FULL JOIN transposition
- **WHEN** a supported FULL JOIN is compiled for PostgreSQL
- **THEN** generated SQL contains two LEFT JOIN branches connected by UNION ALL
  and an IS NULL anti-match predicate, and contains no native FULL JOIN

#### Scenario: FULL JOIN result operators
- **WHEN** a FULL JOIN uses WHERE, DISTINCT, TOP, or final ordering
- **THEN** filtering preserves null-extended row semantics and result-level
  operations apply to the complete transposed result

#### Scenario: Status predicates next to the join key
- **WHEN** ON contains a cross-source equality followed by an IN-list of
  catalog `ЗНАЧЕНИЕ` expressions and an enumeration `ЗНАЧЕНИЕ` comparison
- **THEN** generated SQL retains all three predicates in ON in source order

#### Scenario: Outer join one-sided predicate
- **WHEN** an additional predicate refers only to one source of LEFT, RIGHT, or
  FULL JOIN
- **THEN** it remains in ON and is not moved to WHERE

#### Scenario: FULL JOIN predicate transposition
- **WHEN** FULL JOIN contains additional supported predicates
- **THEN** transposed SQL uses a top-level cross-source equality as its
  anti-match marker and applies the complete ON condition in both branches

#### Scenario: Unsupported join shape
- **WHEN** a query combines FULL JOIN with another join in the same branch,
  references a later source from an earlier join condition, uses wildcard
  joined projection, non-scalar join fields, reference properties in ON,
  lacks a top-level direct-field equality with an earlier source, or nests
  its only equality under OR
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: Repeated query terminator
- **WHEN** a valid query ends in more than one semicolon
- **THEN** all trailing semicolons are consumed as terminators and no
  empty statement is reported

#### Scenario: Bounded syntax failure
- **WHEN** a query contains a mutation, unsupported clause, ambiguous
  reference target, path deeper than one hop, or branch-local ordering
  before another union
- **THEN** compilation returns a positional diagnostic and no SQL is produced

## ADDED Requirements

### Requirement: Compile temporary-table batches
The compiler SHALL accept a batch of statements separated by `;`. A
statement SHALL be a query optionally carrying `ПОМЕСТИТЬ <Имя>` / `INTO`
or `ДОБАВИТЬ <Имя>` / `ADD` after its first selection list and optionally
ending with `ИНДЕКСИРОВАТЬ ПО <поля>` / `INDEX BY`, `ИНДЕКСИРОВАТЬ ПО
НАБОРАМ ((…), …)` / `INDEX BY SETS`, each with an optional `УНИКАЛЬНО` /
`UNIQUE`, or the statement `УНИЧТОЖИТЬ <Имя>` / `DROP`. Temporary tables
SHALL be emulated with common table expressions named `vt1`, `vt2`, … in
definition order: `ПОМЕСТИТЬ` SHALL define a new CTE from the statement
compiled under the nested-query rules (no `*`, no deferred presentations,
ordering only with `ПЕРВЫЕ`), `ДОБАВИТЬ` SHALL define a new CTE
`SELECT … FROM <previous> UNION ALL <statement>` and rebind the name after
a strict positional structure check (equal column count, compatible kinds,
identical reference targets and width, `NULL` compatible with anything),
and `УНИЧТОЖИТЬ` SHALL emit nothing and hide the name so that it MAY be
defined again. `ИНДЕКСИРОВАТЬ ПО` fields SHALL name output labels of the
statement and SHALL generate nothing. A bare identifier source `ИЗ <Имя>
[КАК <Псевдоним>]`, also in joins, nested queries, and `В (ВЫБРАТЬ …)`,
SHALL read a visible temporary table as a derived source whose fields carry
the stored columns and kinds, with the table name as the default alias.
The batch SHALL compile to one statement whose `WITH` list contains
exactly the CTEs reachable from the final statement in ascending order; a
final `ПОМЕСТИТЬ` or `ДОБАВИТЬ` SHALL yield one `Количество` row counting
the rows placed or appended, and a final `УНИЧТОЖИТЬ` SHALL yield no
statement. Temporary-table definitions SHALL stay in the storage date
domain so that the final projection corrects MSSQL dates once. A batch of
more than 64 statements SHALL fail with `WorkBudgetExceeded`, and each
statement SHALL charge the work budget.

#### Scenario: Define and read a temporary table
- **WHEN** a batch is `ВЫБРАТЬ Ссылка КАК Товар, СУММА(Количество) КАК Итог ПОМЕСТИТЬ Обороты ИЗ … СГРУППИРОВАТЬ ПО Ссылка; ВЫБРАТЬ Т.Товар, Т.Итог ИЗ Обороты КАК Т ГДЕ Т.Итог > 0`
- **THEN** both dialects emit `WITH "vt1" AS (…) SELECT … FROM "vt1" AS "Т" WHERE …`
  with the dialect's identifier quoting and the columns `Товар` (reference)
  and `Итог` (number)

#### Scenario: Append rows
- **WHEN** a second statement is `ВЫБРАТЬ Наименование КАК Наименование ДОБАВИТЬ ВТ ИЗ Справочник.Услуги`
  after `ВТ` was placed from `Справочник.Товары` with one string column
- **THEN** the batch defines `vt2 AS (SELECT "Наименование" FROM "vt1" UNION ALL SELECT … )`,
  later reads of `ВТ` use `vt2`, and the `WITH` list carries `vt1` before
  `vt2`

#### Scenario: Structure mismatch on append
- **WHEN** `ДОБАВИТЬ` appends two columns to a one-column table or a
  reference column of another target
- **THEN** compilation fails with a `TemporaryTable` diagnostic at the
  `ДОБАВИТЬ` token and no SQL is produced

#### Scenario: Placement count
- **WHEN** the batch ends with a `ПОМЕСТИТЬ` statement
- **THEN** the generated statement is `WITH … SELECT COUNT(*) AS "Количество" FROM "vtN"`
  with one number column labelled `Количество`

#### Scenario: Drop and redefine
- **WHEN** a batch drops `ВТ` and then places `ВТ` again
- **THEN** the second definition succeeds as a new CTE, a read between the
  drop and the redefinition fails with a `TemporaryTable` diagnostic, and a
  batch ending with the drop yields no statement

#### Scenario: Unreachable definition omitted
- **WHEN** the final statement reads only `ВТ2` while `ВТ1` was placed
  earlier and is not referenced by `ВТ2`
- **THEN** the `WITH` list contains only the CTE of `ВТ2`

#### Scenario: Index clause ignored
- **WHEN** a definition ends with `ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код, Наименование) УНИКАЛЬНО, (Артикул))`
- **THEN** the batch compiles without any index SQL, and a field absent
  from the selection list fails with a `TemporaryTable` diagnostic at that
  field

#### Scenario: Definition restrictions
- **WHEN** a `ПОМЕСТИТЬ` statement projects `*`, requests a deferred
  presentation, orders without `ПЕРВЫЕ`, or appears inside a nested query
- **THEN** compilation fails with a positional diagnostic

### Requirement: Manage temporary tables across compilations
The `open-sdbl` library SHALL provide `TempTablesManager`, a plain value
holding compiled temporary-table definitions (name, CTE name, SQL,
columns, dependencies, visibility) that survives between compilations.
`QueryCompiler::compile_batch(source, &options, &mut manager)` and
`Prepared::compile_batch(snapshot, &options, &mut manager)` SHALL read
visible tables from the manager, SHALL return `Ok(None)` when the batch
ends with `УНИЧТОЖИТЬ`, and SHALL commit the batch's definitions and drops
to the manager only when compilation succeeds.
`QueryCompiler::prepare_with(source, &manager)` SHALL collect presentation
targets with the manager's tables visible. The manager SHALL expose the
visible tables with their names and `CompiledColumn`s, `contains`,
`is_empty`, and `clear`. The first definition SHALL bind the manager to the
compiling dialect and snapshot; use with another dialect SHALL fail with
`TemporaryTable` and with another snapshot with `SnapshotMismatch`. A
manager SHALL hold at most 256 definitions. `compile`, `compile_with`,
`prepare`, `compile_with_presentations`, and `Prepared::compile` SHALL
accept batches with a manager private to the call and SHALL fail with a
`TemporaryTable` diagnostic when the batch returns no rows.

#### Scenario: Definition reused by a later batch
- **WHEN** an application compiles `ВЫБРАТЬ … ПОМЕСТИТЬ ВТ ИЗ … ГДЕ Дата > &Период`
  with a bound `Период` and then compiles `ВЫБРАТЬ Т.Дата ИЗ ВТ КАК Т`
  with the same manager and no parameters
- **THEN** the second SQL contains the first definition with the inlined
  date literal and no unused-parameter diagnostic is raised

#### Scenario: Failed batch leaves the manager unchanged
- **WHEN** a batch places `ВТ1` and then fails on its second statement
- **THEN** the manager still lists the tables it had before the call

#### Scenario: Manager listing
- **WHEN** an application iterates `manager.tables()` after placing and
  appending to `ВТ`
- **THEN** it sees one entry `ВТ` with the columns and kinds of the first
  definition

#### Scenario: Batch without rows through compile
- **WHEN** an application calls `compile` on `УНИЧТОЖИТЬ ВТ`
- **THEN** compilation fails with a `TemporaryTable` diagnostic

### Requirement: Manage console temporary tables
The console SHALL keep one `TempTablesManager` for the session, SHALL run
every statement through `prepare_with` and `Prepared::compile_batch`, SHALL
execute and display the statement when one is produced (so a `ПОМЕСТИТЬ`
statement shows its `Количество` row), SHALL print the names dropped from
the manager when none is produced, SHALL provide `\tables` listing every
visible temporary table with its name and columns as label and kind, SHALL
clear the manager with a notice on `\refresh`, and SHALL offer visible
table names in source completion.

#### Scenario: Batch entered statement by statement
- **WHEN** the user enters a `ПОМЕСТИТЬ` statement terminated by `;` and
  then a query reading that table terminated by `;`
- **THEN** the first shows a `Количество` row, the second executes the
  `WITH` statement, and `\tables` lists the table in between

#### Scenario: Drop notice
- **WHEN** the user enters `УНИЧТОЖИТЬ ВТ;`
- **THEN** the console prints that `ВТ` was dropped, executes no SQL, and
  `\tables` no longer lists it

#### Scenario: Refresh clears definitions
- **WHEN** the user enters `\refresh` after placing a table
- **THEN** the console reloads metadata, prints that temporary tables were
  cleared, and `\tables` reports none
