## ADDED Requirements

### Requirement: Walk reference paths of any depth
A field path SHALL dereference any number of single-target references,
joining each hop's target to the alias the previous hop produced and
reading the last segment from the table the walk ended on. Identical hops
of one branch SHALL share one join. The path SHALL work wherever a
one-hop path does: projections, `ГДЕ`, `СГРУППИРОВАТЬ ПО` and
`УПОРЯДОЧИТЬ ПО`. A path that continues through a composite reference
SHALL be an `UnsupportedFeature` diagnostic naming that field, because
such a reference selects its value by type and has no single table to
continue from.

#### Scenario: Two hops
- **WHEN** `ВЫБРАТЬ Т.Поставщик.Родитель.Наименование ИЗ Справочник.Товары КАК Т`
  is executed
- **THEN** the answer is the name of the supplier's folder, and `NULL`
  where either reference is empty

#### Scenario: Shared prefix
- **WHEN** a query reads `Т.Поставщик.Родитель.Наименование` and
  `Т.Поставщик.Родитель.Код`
- **THEN** the generated SQL joins the supplier once and its folder once

#### Scenario: Composite in the middle
- **WHEN** the path continues through a composite reference
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
