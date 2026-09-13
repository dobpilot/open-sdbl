## ADDED Requirements

### Requirement: Compute query totals
The compiler SHALL accept `ИТОГИ [<поле> [КАК Псевдоним], …] ПО [ОБЩИЕ]
[<контрольная точка> [ПЕРИОДАМИ(<период>[, <дата>[, <дата>]])] [КАК
Псевдоним], …]` / `TOTALS … BY [OVERALL] …` at the end of a top-level
statement and SHALL return the rows of the platform's linear traversal:
the overall row when `ОБЩИЕ` is present, then for every value of the
first control point one total row followed by the rows of that group,
recursively for deeper control points. Groups SHALL be ordered by the
first appearance of their value in the statement's ordered result and
detail rows SHALL keep that order inside their group. A total row SHALL
carry the control points of its own and enclosing levels, the totals
fields aggregated over the result columns of its group, and `NULL` in
every other column. A totals field SHALL be an expression over
aggregates of result columns; a bare aggregate targets its argument
column, an expression targets the column its alias names, an alias
naming no result column SHALL be a `Syntax` diagnostic, and a later
field naming the same column wins. `ПЕРИОДАМИ` SHALL be validated (a
date control point, date literals or parameters) and SHALL not alter the
rows. `ИТОГИ` with `ПОМЕСТИТЬ`/`ДОБАВИТЬ` SHALL be a `Syntax` diagnostic,
`ИТОГИ` inside a nested query and a control point that is not a result
column SHALL be `UnsupportedFeature` diagnostics.
`CompileOptions::totals_level(true)` SHALL append a numeric `__level`
column equal to the platform's `Уровень()`; the console SHALL enable it.

#### Scenario: Overall and one level
- **WHEN** `ВЫБРАТЬ Т.Родитель КАК Родитель, Т.Наименование КАК Имя,
  Т.Цена КАК Цена ИЗ … УПОРЯДОЧИТЬ ПО Т.Цена УБЫВ ИТОГИ СУММА(Цена) ПО
  ОБЩИЕ, Родитель` is executed
- **THEN** the first row has `NULL` parent and name and the total price,
  each parent's total row precedes its items, parents appear in the
  order of their most expensive item, and the level column reads `0`,
  `1`, `2`

#### Scenario: Totals without fields
- **WHEN** `ИТОГИ ПО Родитель` is executed
- **THEN** each total row carries the parent and `NULL` in the other
  columns

#### Scenario: Union and TOP inputs
- **WHEN** the statement has `ОБЪЕДИНИТЬ ВСЕ` or `ПЕРВЫЕ 3` and totals
- **THEN** the totals are computed over the union rows, or the three
  selected rows, respectively

#### Scenario: Temporary table
- **WHEN** `ПОМЕСТИТЬ ВТ … ИТОГИ СУММА(Цена) ПО ОБЩИЕ` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the `ИТОГИ`
  token

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
[OUTER] JOIN`. The source list MAY contain several comma-separated
elements, each a source with its own joins; every element after the
first SHALL be rendered as a `CROSS JOIN` in written order, SHALL be
filtered like the base source, and a join condition SHALL see only the
sources of its own element (`UnknownField` otherwise). A comma list
SHALL keep the `FULL JOIN` and `*` refusals of joined branches. Joined
branches SHALL support named direct fields and one-hop
reference properties. Each join condition SHALL contain at least one top-level
scalar equality between a direct field or one-hop reference property of the
joined source and one of an earlier source, and MAY combine that anchor with
additional supported scalar predicates over direct fields and one-hop
reference properties of the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. A `ПОЛНОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` condition SHALL use direct fields only. Final `УПОРЯДОЧИТЬ
ПО`/`ORDER BY` SHALL support `ВОЗР`/`ASC` and `УБЫВ`/`DESC` and SHALL
accept a projection alias of the branch as a key, ordering by the aliased
expression. Statements
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
  joined projection, non-scalar join fields, reference paths deeper than
  one hop in ON, a reference property in a FULL JOIN condition, lacks a
  top-level equality with an earlier source, or nests its only equality
  under OR
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

#### Scenario: Comma-separated sources
- **WHEN** a branch reads `ИЗ Справочник.А КАК А, Справочник.Б КАК Б
  ГДЕ А.Поле = Б.Поле`
- **THEN** generated SQL is `FROM … AS "А" CROSS JOIN … AS "Б" WHERE …`
  on both providers

#### Scenario: Join inside a comma element
- **WHEN** a branch reads `ИЗ А, Б ЛЕВОЕ СОЕДИНЕНИЕ В ПО В.x = Б.y`
- **THEN** the `LEFT JOIN` follows the `CROSS JOIN` in the written order,
  and a condition naming a field of `А` is an `UnknownField` diagnostic

#### Scenario: Ordering by a projection alias
- **WHEN** `ВЫБРАТЬ ГОД(Дата) КАК Год ИЗ … УПОРЯДОЧИТЬ ПО Год УБЫВ` is
  compiled without joins or grouping
- **THEN** generated SQL orders by the year expression
