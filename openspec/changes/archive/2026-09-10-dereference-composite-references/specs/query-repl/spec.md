## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Dereference composite reference fields
The compiler SHALL accept a one-hop dereference through a composite
reference field (`_RTRef` and `_RRRef` members) and through a runtime-typed
column of a derived source or temporary table. Candidate targets SHALL be
the field's declared SchemaStorage targets when present, otherwise every
reference-kind metadata object whose fields include the named attribute.
Candidates without the attribute SHALL be skipped; no candidate SHALL fail
with `UnknownField`; more than 32 candidates SHALL fail with
`UnsupportedFeature` naming `ВЫРАЗИТЬ`. Each candidate SHALL be joined with
a `LEFT JOIN` guarded by its type number through the shared join key, and
the value SHALL be a `CASE` over the reference type selecting the
candidate's column, yielding `NULL` for rows of other types. The result
kind SHALL be the common kind of the attribute across candidates: equal
variants (else a positional diagnostic), the widest string length, number
without precision, references widened to a runtime-typed payload with the
union of targets. Presentation of the value and a second hop SHALL fail
with `UnsupportedFeature`.

#### Scenario: Any-reference field dereferenced by attribute scan
- **WHEN** a query projects `Связь.СвязанныйОбъект.РегистрационныйНомер`
  from a register whose `СвязанныйОбъект` declares no targets and two
  catalogs define `РегистрационныйНомер`
- **THEN** both dialects left-join each catalog on `_RRRef` with an `_RTRef`
  type guard and project `CASE WHEN _RTRef = <type1> THEN … WHEN _RTRef =
  <type2> THEN … END` as a string column

#### Scenario: Declared multi-target field
- **WHEN** a query filters on `Регистратор.Номер` of a register whose
  recorder declares three document targets
- **THEN** only the declared documents are joined, each with its type
  guard, and the filter compares the `CASE` value

#### Scenario: Temporary-table payload column
- **WHEN** a temporary table placed from a composite field is read with
  `Т.Ссылка.Наименование`
- **THEN** the payload column is split into its type and identifier parts
  for the guarded joins and the `CASE` value

#### Scenario: Too many candidates
- **WHEN** an any-reference field is dereferenced to an attribute defined
  by more than 32 objects
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic that
  suggests narrowing the field with `ВЫРАЗИТЬ`
