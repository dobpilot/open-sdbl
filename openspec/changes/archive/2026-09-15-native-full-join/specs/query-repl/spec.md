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
`RIGHT [OUTER] JOIN`, and `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` / `FULL
[OUTER] JOIN` in source order, each introducing one more source scope. The source list MAY contain several comma-separated
elements, each a source with its own joins; every element after the
first SHALL be rendered as a `CROSS JOIN` in written order, SHALL be
filtered like the base source, and a join condition SHALL see only the
sources of its own element (`UnknownField` otherwise). A comma list
SHALL keep the `*` refusals of joined branches. Joined
branches SHALL support named direct fields and one-hop
reference properties. Each join condition SHALL contain at least one top-level
scalar equality between a direct field or one-hop reference property of the
joined source and one of an earlier source, and MAY combine that anchor with
additional supported scalar predicates over direct fields and one-hop
reference properties of the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. A `ПОЛНОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` condition MAY dereference, and the reference join it needs
SHALL be resolved inside the side that owns it. Final `УПОРЯДОЧИТЬ
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
- **WHEN** a FULL JOIN is compiled
- **THEN** no transposition is generated: the SQL carries a native `FULL
  JOIN` between the two sources, and no UNION ALL with an anti-match
  predicate

#### Scenario: FULL JOIN in a chain
- **WHEN** a branch chains a LEFT JOIN and a FULL JOIN in either order, or
  two FULL JOINs
- **THEN** every join is rendered in source order and the result matches
  the platform row for row

#### Scenario: FULL JOIN under aggregation
- **WHEN** a branch aggregates or groups over a FULL JOIN
- **THEN** the aggregate sees the null-extended rows of both sides, as the
  platform answers

#### Scenario: FULL JOIN condition that dereferences
- **WHEN** a FULL JOIN condition compares `Т.Клиент.Наименование` with a
  field of the joined source
- **THEN** the reference join is placed inside the side that owns it, so it
  cannot land after the full join

#### Scenario: FULL JOIN condition that cannot be planned
- **WHEN** a FULL JOIN condition is not an equality chain
- **THEN** compilation fails, because the server plans a full join only on
  merge- or hash-joinable conditions

#### Scenario: FULL JOIN result operators
- **WHEN** a FULL JOIN uses WHERE, DISTINCT, TOP, or final ordering
- **THEN** filtering preserves null-extended row semantics and result-level
  operations apply to the whole joined result

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

### Requirement: Compile the scalar string and arithmetic functions
The compiler SHALL accept `ПОДСТРОКА(x, n, m)`, `ДлинаСтроки(x)`,
`СокрЛП(x)`, `СокрЛ(x)`, `СокрП(x)`, `ВРег(x)`, `НРег(x)`, `Лев(x, n)`,
`Прав(x, n)`, `СтрНайти(x, s)`, `СтрЗаменить(x, s, r)`, `Окр(x[, n])`,
`Цел(x)`, `Sqrt`, `Exp`, `Log`, `Log10`, `Pow`, `Cos`, `Sin`, `Tan`,
`ACos`, `ASin`, and `ATan`, with their English spellings, and SHALL
render each per dialect. String functions SHALL return a string and the
others a number. An argument of the wrong kind SHALL be a `Syntax`
diagnostic, `NULL` and unclassified values passing. The arity SHALL be
checked at parse time. PostgreSQL renderings SHALL stay within the targeted
minimum server,
and a character column of the 1C extension types SHALL be cast to `text`
before the call. `Log` SHALL be the natural logarithm, `Log10` the
decimal one, `Окр` SHALL round half away from zero and accept a negative
scale, and `Цел` SHALL truncate toward zero, as measured on the platform.

#### Scenario: String functions
- **WHEN** `ПОДСТРОКА(Т.Наименование, 2, 3)`, `Лев(Т.Наименование, 3)`,
  and `СтрНайти(Т.Наименование, "ан")` are compiled
- **THEN** PostgreSQL renders `substring(… from 2 for 3)`,
  `substring(… from 1 for 3)`, and `position('ан' in …)`, and the results
  match the platform row for row

#### Scenario: Trailing spaces
- **WHEN** `ДлинаСтроки("  а  ")` is compiled
- **THEN** the answer is 5 on both providers, so SQL Server cannot use
  `LEN` alone

#### Scenario: Wrong argument kind
- **WHEN** `ВРег(Т.Цена)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic naming the
  expected kind
