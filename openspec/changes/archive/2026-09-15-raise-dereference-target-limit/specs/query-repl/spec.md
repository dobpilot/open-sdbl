## MODIFIED Requirements

### Requirement: Dereference composite reference fields
The compiler SHALL accept a one-hop dereference through a composite
reference field (`_RTRef` and `_RRRef` members) and through a runtime-typed
column of a derived source or temporary table. Candidate targets SHALL be
the field's declared SchemaStorage targets when present, otherwise every
reference-kind metadata object whose fields include the named attribute.
Candidates without the attribute SHALL be skipped; no candidate SHALL fail
with `UnknownField`; more candidates than the statement can carry — 256,
the number of tables SQL Server accepts in one statement — SHALL fail with
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

#### Scenario: Many candidates
- **WHEN** an any-reference field of a real configuration is dereferenced
  to an attribute defined by 94 objects
- **THEN** every candidate is joined and the server plans the statement,
  as the platform answers such a query

#### Scenario: Too many candidates
- **WHEN** an any-reference field is dereferenced to an attribute defined
  by more objects than one statement can join
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic that
  suggests narrowing the field with `ВЫРАЗИТЬ`
