## ADDED Requirements

### Requirement: Resolve the computed standard fields
`ЭтоГруппа` / `IsFolder` SHALL resolve to the negation of the stored
`Folder` column, and `Предопределенный` SHALL resolve to the
`PredefinedID` column differing from the empty reference, because the
platform computes both rather than storing them. They SHALL answer in a
projection, a predicate, a grouping key, an ordering key and through a
reference, SHALL carry the boolean column kind, and SHALL be spelled as a
bit on SQL Server.

#### Scenario: Folders of a catalog
- **WHEN** `ВЫБРАТЬ Т.ЭтоГруппа ИЗ Справочник.Товары КАК Т ГДЕ Т.ЭтоГруппа`
  is executed over a hierarchical catalog
- **THEN** only the folders answer, with the value true, exactly as on
  the platform

#### Scenario: Predefined items
- **WHEN** `ВЫБРАТЬ Т.Предопределенный ИЗ Справочник.Товары КАК Т` is
  executed
- **THEN** only the predefined items answer true

#### Scenario: Through a reference
- **WHEN** the field is read as `Т.Поставщик.ЭтоГруппа`
- **THEN** it answers from the joined table, and `NULL` where the
  reference is empty
