## MODIFIED Requirements

### Requirement: Unaliased fields are labelled as written
A projected field without `КАК` SHALL carry the label of its path
segments after the source alias, as the text spells them and run
together — `Ссылка` for `Т.Ссылка`, `Ref` for `Т.Ref`, `Регистратор`
for `Д.Регистратор`, `ОрганизацияНаименование` for
`Д.Организация.Наименование` — with the member suffixes of a compound
field appended; a nested query or temporary table exposes the column
under that name.

#### Scenario: Temporary table read by the written name
- **WHEN** `ВЫБРАТЬ Д.Регистратор ПОМЕСТИТЬ ВТ ИЗ … КАК Д; ВЫБРАТЬ ВТ.Регистратор ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the table's column is `Регистратор` and the second statement
  reads it

#### Scenario: Dereferenced path read by the run-together name
- **WHEN** `ВЫБРАТЬ Д.Организация.Наименование ПОМЕСТИТЬ ВТ ИЗ … КАК Д; ВЫБРАТЬ ВТ.ОрганизацияНаименование ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the table's column is `ОрганизацияНаименование` and the
  second statement reads it

### Requirement: Compile nested query sources
The compiler SHALL accept `(<query>) [КАК] <alias>` as a source in `ИЗ` and
in any join, compiling the nested query as an independent statement whose
output columns become the derived source's fields with their column kinds.
A nested query MAY use unions, grouping, joins, `ПЕРВЫЕ`, `РАЗЛИЧНЫЕ`,
inline presentations, and final ordering when every branch has `ПЕРВЫЕ`,
but SHALL NOT contain ordering over a branch without `ПЕРВЫЕ`, deferred
reference presentations, or `*`. The ordering of a nested union SHALL be
dropped: each branch limits itself and the union has no limit of its own,
so the order changes nothing. A
derived column whose kind is a fixed single-target reference SHALL support
one-hop dereference through the shared join cache; a runtime-typed derived
column SHALL be dereferenced under the composite-reference rules with its
known targets as the declared candidates. Identifiers that resolve only in an enclosing query SHALL
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

#### Scenario: Nested union of limited branches
- **WHEN** `ВЫБРАТЬ ПЕРВЫЕ 10 Д.Ссылка КАК Ссылка ПОМЕСТИТЬ ВТ ИЗ Документ.X КАК Д ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ ПЕРВЫЕ 10 Д.Ссылка ИЗ Документ.Y КАК Д УПОРЯДОЧИТЬ ПО Ссылка;`
  is compiled
- **THEN** the definition compiles with a limit on each branch and no
  `ORDER BY`

#### Scenario: Nested union with an unlimited branch
- **WHEN** one branch of an ordered nested union has no `ПЕРВЫЕ`
- **THEN** compilation fails with a positional diagnostic

### Requirement: Test reference types with the REFS operator
The compiler SHALL accept `<поле> ССЫЛКА <Вид>.<Объект>` /
`<field> REFS <Kind>.<Object>` as a predicate wherever comparisons are
accepted, the operand being a direct field, a one-hop reference
property, or `ВЫРАЗИТЬ(<поле> КАК <Вид>.<Объект>)` naming the same
target as the operator — such a cast keeps a reference of that type and
turns any other into NULL, so the test is the field's. For a composite reference field it SHALL compare the field's
type member with the target's database type number; for a runtime-typed
nested-query column it SHALL compare the first four payload bytes; for a
fixed-target field of the named table it SHALL be a constant true
predicate, which also holds for the empty reference. A fixed-target
field of another table and a non-reference operand SHALL be `Syntax`
diagnostics, and a source-free statement SHALL refuse the operator.

#### Scenario: Composite attribute
- **WHEN** `ГДЕ Т.Объект ССЫЛКА Справочник.Товары` is compiled for a
  composite attribute
- **THEN** generated SQL compares `Т._Fld<N>_RTRef` with the catalog's
  type number on both providers

#### Scenario: Fixed-target attribute
- **WHEN** `ГДЕ Т.Клиент ССЫЛКА Справочник.Клиенты` is compiled for an
  attribute typed with that catalog only
- **THEN** generated SQL contains a true predicate, and naming another
  catalog fails with a `Syntax` diagnostic

#### Scenario: Value position
- **WHEN** `ВЫБОР КОГДА Т.Объект ССЫЛКА Справочник.Товары ТОГДА 1 ИНАЧЕ 0 КОНЕЦ`
  is compiled
- **THEN** the type test is the `CASE` condition

#### Scenario: Cast operand
- **WHEN** `ГДЕ ВЫРАЗИТЬ(Т.Объект КАК Справочник.Товары) ССЫЛКА Справочник.Товары`
  is compiled for a composite attribute
- **THEN** generated SQL compares the attribute's type member with the
  catalog's type number, as the uncast field would
