## ADDED Requirements

### Requirement: Join conditions without an anchor equality
An inner, left or right join SHALL accept any condition over the joined
source and earlier ones — a constant, a comparison with a parameter, an
inequality, `МЕЖДУ`, a dereference through `ВЫРАЗИТЬ` — rendered as the
`ON` predicate; a full join SHALL keep requiring a top-level direct-field
equality between the joined source and an earlier source, reported as
an `UnsupportedFeature` diagnostic naming the full join.

#### Scenario: Left join on a constant
- **WHEN** `… ЛЕВОЕ СОЕДИНЕНИЕ ПланСчетов.Хозрасчетный КАК Х ПО (ИСТИНА)` is compiled
- **THEN** the SQL joins with `ON TRUE`

#### Scenario: Cast dereference in the condition
- **WHEN** the condition reads `ВЫРАЗИТЬ(Д.Регистратор КАК Документ.X).Поле = Т.Поле`
- **THEN** the target of the cast is joined before the condition's join

#### Scenario: Full join without an anchor
- **WHEN** `… ПОЛНОЕ СОЕДИНЕНИЕ … ПО (ИСТИНА)` is compiled
- **THEN** the diagnostic is `UnsupportedFeature`
