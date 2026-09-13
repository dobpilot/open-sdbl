## ADDED Requirements

### Requirement: Compile hierarchy membership
The compiler SHALL accept `<поле> [НЕ] В ИЕРАРХИИ (<список> | <запрос>)` /
`<field> [NOT] IN HIERARCHY (…)` wherever `В` is accepted. The predicate
SHALL be true when the value equals one of the seeds or descends from one
through the catalog's `_ParentIDRRef` chain, and `НЕ` SHALL negate it.
The descent SHALL be rendered as one recursive CTE per predicate, defined
at statement level — `WITH RECURSIVE` on PostgreSQL, `WITH` on SQL Server
— and tested with `EXISTS`, so a `NULL` seed cannot swallow the result. A
target catalog without a live parent column SHALL degenerate to plain
membership. The tested value SHALL be a field referencing exactly one
catalog, otherwise an `UnsupportedFeature` diagnostic; seeds SHALL be a
nested query or constants, and a field among them SHALL be an
`UnsupportedFeature` diagnostic. A statement that defines a temporary
table SHALL refuse the predicate.

#### Scenario: Folder subtree
- **WHEN** `ГДЕ Т.Ссылка В ИЕРАРХИИ (ВЫБРАТЬ Г.Ссылка ИЗ Справочник.Товары КАК Г ГДЕ Г.Наименование = "Мебель")`
  is executed over a catalog whose `Мебель` folder holds `Кухня`
- **THEN** the rows are `Мебель`, `Кухня`, and every item beneath them

#### Scenario: Item seed
- **WHEN** the seed is an item rather than a folder
- **THEN** only that item is selected

#### Scenario: Empty reference
- **WHEN** the seed is the empty reference of the catalog
- **THEN** every row of the catalog is selected, because every chain of
  parents ends at the empty reference

#### Scenario: Catalog without a hierarchy
- **WHEN** the target catalog has no parent column
- **THEN** the predicate is plain membership in the seeds
