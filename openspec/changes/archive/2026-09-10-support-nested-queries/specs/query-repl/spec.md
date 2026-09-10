## ADDED Requirements

### Requirement: Compile nested query sources
The compiler SHALL accept `(<query>) [КАК] <alias>` as a source in `ИЗ` and
in any join, compiling the nested query as an independent statement whose
output columns become the derived source's fields with their column kinds.
A nested query MAY use unions, grouping, joins, `ПЕРВЫЕ`, `РАЗЛИЧНЫЕ`,
inline presentations, and final ordering together with `ПЕРВЫЕ`, but SHALL
NOT contain ordering without `ПЕРВЫЕ`, deferred reference presentations, or
`*`. A
derived column whose kind is a fixed single-target reference SHALL support
one-hop dereference through the shared join cache; a runtime-typed derived
column SHALL NOT. Identifiers that resolve only in an enclosing query SHALL
fail with a positional diagnostic. Nested statements SHALL count toward the
parser depth limit and the work budget.

#### Scenario: Grouped nested source joined to a catalog
- **WHEN** a query joins `(ВЫБРАТЬ Номенклатура, СУММА(Количество) КАК Итог ИЗ … СГРУППИРОВАТЬ ПО Номенклатура) КАК Т`
  to `Справочник.Номенклатура` on `Т.Номенклатура = Н.Ссылка`
- **THEN** both dialects emit the nested statement as a parenthesized derived
  table with the alias, and `Т.Итог` is projected as a number column

#### Scenario: Dereference through a derived reference
- **WHEN** the outer query projects `Т.Номенклатура.Наименование`
- **THEN** generated SQL left-joins the catalog on the derived reference
  column and projects its description

#### Scenario: First N as a source
- **WHEN** a nested source is `ВЫБРАТЬ ПЕРВЫЕ 10 Ссылка, Дата ИЗ Документ.Заказ УПОРЯДОЧИТЬ ПО Дата УБЫВ`
- **THEN** MSSQL emits `TOP (10) … ORDER BY` and PostgreSQL emits
  `ORDER BY … LIMIT 10` inside the derived table

#### Scenario: Correlated reference
- **WHEN** a nested query's filter names a field of the outer source
- **THEN** compilation fails with a positional diagnostic at that field

### Requirement: Compile subquery membership predicates
The compiler SHALL accept `<expr> [НЕ] В (<query>)` / `[NOT] IN (SELECT …)`
where the nested query projects exactly one column of a kind compatible
with the left operand, and `<expr> НЕ В (<list>)`. Scalar operands SHALL
compile to `[NOT] IN (…)`. Reference operands SHALL compare `RRRef`
members: a runtime-typed left operand against a fixed-target subquery SHALL
add an `RTRef` guard, a fixed-target left operand against a runtime-typed
subquery SHALL filter the subquery by that `RTRef`, and two runtime-typed
sides SHALL compare payloads. A subquery with more than one column SHALL fail
with a positional diagnostic.

#### Scenario: Reference membership
- **WHEN** a query filters with `Номенклатура В (ВЫБРАТЬ Ссылка ИЗ Справочник.Номенклатура ГДЕ ПометкаУдаления)`
  on a fixed-target field
- **THEN** generated SQL compares the field's `RRRef` with `IN (SELECT …)`
  over the catalog `_IDRRef`

#### Scenario: Guarded membership
- **WHEN** the left operand is a runtime-typed reference and the subquery
  projects catalog references
- **THEN** generated SQL wraps the `IN` predicate with an `RTRef` equality
  for that catalog

#### Scenario: Negated list
- **WHEN** a query filters with `Код НЕ В ("1", "2")`
- **THEN** generated SQL contains `NOT IN ('1', '2')`
