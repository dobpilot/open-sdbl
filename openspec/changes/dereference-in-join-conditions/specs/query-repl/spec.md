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
scalar equality between a direct field or one-hop reference property of the
joined source and one of an earlier source, and MAY combine that anchor with
additional supported scalar predicates over direct fields and one-hop
reference properties of the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. A `ПОЛНОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` condition SHALL use direct fields only. Final `УПОРЯДОЧИТЬ
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

## ADDED Requirements

### Requirement: Dereference references inside join conditions
The compiler SHALL accept one-hop dereferences through fixed single-target
references of the joined source and of earlier sources anywhere in a join
condition. When a condition uses a dereference, the generated SQL SHALL
render the dereference joins of every source as a parenthesized group next
to that source's relation so that every `ON` clause can reference them;
statements whose conditions use no dereference SHALL keep the flat join
list. A `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` whose condition dereferences a
reference SHALL fail with a positional diagnostic.

#### Scenario: Tabular section joined through its owner
- **WHEN** a query joins `Документ.Корреспонденция.Корреспонденты КАК К`
  to `Справочник.ДокументыПредприятия КАК Д` on `К.Ссылка.Основание = Д.Ссылка`
- **THEN** both dialects emit the tabular section grouped with a `LEFT JOIN`
  of its owner document and compare the owner's `Основание` column with the
  catalog identifier in the `ON` clause

#### Scenario: Earlier source dereferenced in a later condition
- **WHEN** the third source of a chain is joined on a property of the first
  source's reference field
- **THEN** the first source is rendered as a group with its dereference
  join and the third `ON` references that join's alias

#### Scenario: Flat rendering preserved
- **WHEN** no join condition dereferences a reference
- **THEN** the generated SQL is identical to the SQL generated before this
  change
