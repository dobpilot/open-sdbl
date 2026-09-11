## Context

The library has one application callback today: `prepare` collects a
`PresentationRequest`, the application answers with `PresentationPlan`s, and
`Prepared::compile_with` recompiles the source. Row-level restrictions need
the same shape because the core crate does no I/O and the application must
be free to look up the user's rights asynchronously between the two phases.

The compiler already turns every metadata source into a relation string
(`compile_source_relation`): a quoted table, a `UNION ALL` of extension
tables, or a virtual-table subquery. Restricting a source therefore means
producing a different relation string for the same scope, which keeps the
rest of the pipeline untouched.

## Decisions

### Keyword scope

`ВЫБРАТЬ РАЗРЕШЕННЫЕ [РАЗЛИЧНЫЕ] [ПЕРВЫЕ n]` in that order. The parser
hoists the token to `QueryAst::allowed` like `ПОМЕСТИТЬ`; a later union
branch or a nested query carrying it is a `Syntax` diagnostic. The flag
applies to every source the statement reads, including nested sources and
`В (ВЫБРАТЬ …)` subqueries, and is stored on the statement's
`CompilationCatalog` so nested compilations inherit it without threading a
parameter. Statements of a batch are independent: `ПОМЕСТИТЬ` under the
keyword stores already-filtered rows, and a later read of the temporary
table is not filtered again.

### Public value model

```rust
pub struct RestrictionTarget { pub object: ObjectId, pub table_part: Option<String> }
pub struct RestrictionRequest { pub targets: Vec<RestrictionTarget> }   // sorted, deduplicated
pub struct AccessRestriction { object: ObjectId, table_part: Option<String>, condition: String }
impl AccessRestriction { fn new(object, condition) -> Self; fn table_part(self, name) -> Self; getters }
pub struct SessionParameters { values: Vec<QueryParameter> }
impl SessionParameters { fn new(); fn set(&mut self, QueryParameter); fn remove(&mut self, name) -> bool; fn get(&self, name); fn iter(); fn is_empty() }
impl CompileOptions<'a> { fn restrictions(self, &'a [AccessRestriction]) -> Self; fn session(self, &'a SessionParameters) -> Self; getters }
impl Prepared<B> { fn restriction_request(&self) -> &RestrictionRequest }
```

A tabular section is addressed by its owner object plus the section name
(compared case-insensitively), matching how the query spells it
(`Документ.Реализация.Товары`). Service sections (`Изменения` and the
calculation-kind tables) are reported the same way.

### Restriction text

The body is parsed with the SDBL lexer and the ordinary expression parser
(`Parser::parse_condition`: one boolean expression, nothing after it) and
compiled with a `CompilationContext` holding a single source scope whose
alias is `__restricted`, so field names resolve exactly as in a `ГДЕ` over
`ИЗ <target>`. Dereferences add `LEFT JOIN`s inside the wrapper; nested
`В (ВЫБРАТЬ …)` compiles through the same catalog with the keyword flag
cleared, so the tables a restriction reads are never restricted themselves
(as the platform runs restriction templates with full rights).

Parameters: the catalog's `Parameters` value is swapped for the duration
of the restriction compile to a value that sees only session parameters.
`Parameters` gains a `session` slice; `lookup` searches query values first,
then session values.

### Rendering

Plain sources (catalogs, documents, tabular sections, plain register reads,
every other live table, with or without extension `UNION ALL`):

```sql
(SELECT "__restricted"."_IDRRef" AS "_IDRRef", … FROM <relation> AS "__restricted"
   [LEFT JOIN … AS "__ref1" ON …] WHERE <condition>) AS <alias>
```

The projection lists every physical column of the scope's queryable fields
(deduplicated), so the outer statement sees the same columns as before.
The wrapper is its own SQL scope, so `__restricted` and the dereference
aliases inside it cannot clash with the outer query. The same wrapper is
used for `FROM`, `INNER`, `LEFT`, `RIGHT`, and `FULL` joins.

Virtual tables: the restriction is compiled with the virtual table's own
scope (`__slice_base`, `__totals_base`, `__movement_base`,
`__aggregate_base`) and conjoined to its predicate list; the existing
"direct fields only" rule of the condition applies, reported as a
`Restriction` diagnostic.

### Request and validation

`CompilationCatalog` records every `(object, table_part)` a restricted
statement reads and which supplied restrictions matched. After the batch,
in strict mode, a restriction that matched nothing raises `Restriction`
("supplied but no ALLOWED statement reads it"); two restrictions for the
same target raise `Restriction` before compilation. Preparation runs
without restrictions and collects the request in sorted order.

### Diagnostics

`QueryDiagnosticKind::Restriction` wraps any lexer, parser, resolution, or
generation failure inside a restriction: the message is
`restriction of <Kind>.<Name>[.<Section>]: <inner message>` and the position
is the position inside the restriction text. The enum is
`#[non_exhaustive]`, so callers keep their fallback arm.

## Risks / Trade-offs

- Wrapping defeats nothing for the planner: PostgreSQL and SQL Server pull
  predicates through derived tables. The SQL is longer, which is accepted
  for uniformity across join kinds.
- Register restrictions through virtual tables are limited to direct
  dimensions, as in the platform. Plain register reads are unrestricted in
  capability.
- Dereferenced tables stay unfiltered; a follow-up change will make
  restricted dereferences yield `NULL`.
