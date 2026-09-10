## ADDED Requirements

### Requirement: Widen reference equalities in join conditions
A join equality whose operands are references of different width SHALL
compare `RTRef ‖ RRRef` payloads: a fixed single-target reference SHALL be
widened to its target type number followed by its identifier, a composite
field SHALL be widened to the concatenation of its `_RTRef` and `_RRRef`
members, and a runtime-typed column of a derived source or temporary table
SHALL be used as it is. Operands of equal width SHALL keep their direct
comparison. The widened expression SHALL serve as the join anchor marker.

#### Scenario: Fixed reference joined to a temporary-table column
- **WHEN** a catalog is joined to a temporary table on `Д.Ссылка = П.Ссылка`
  where `П.Ссылка` was placed from a composite register field
- **THEN** both dialects compare the catalog's type number concatenated
  with `_IDRRef` against the payload column, and the join matches rows

#### Scenario: Composite field joined to a derived column
- **WHEN** a register's composite `Объект` is joined to a grouped nested
  source's `Документ` column that was projected from the same composite field
- **THEN** generated SQL compares `_RTRef ‖ _RRRef` with the derived column
  instead of failing with a missing-target diagnostic

#### Scenario: Payload against payload
- **WHEN** two temporary tables are joined on runtime-typed columns
- **THEN** generated SQL compares the two columns directly
