## ADDED Requirements

### Requirement: Resolve the predefined-data name
`ИмяПредопределенныхДанных` / `PredefinedDataName` SHALL resolve to the
symbolic name of the predefined item a row is, taken from the predefined
values of the owning object, and to an empty string for a row that is not
predefined, because the platform derives the name from the stored
`PredefinedID` rather than storing it. It SHALL answer in a projection, a
predicate, a grouping key and an ordering key, SHALL carry the string
column kind, and SHALL be refused on an object without a `PredefinedID`
column. Through a reference it SHALL answer from the joined table and
stay `NULL` where the reference matched no row.

#### Scenario: Predefined items of a catalog
- **WHEN** `ВЫБРАТЬ Т.ИмяПредопределенныхДанных ИЗ Справочник.Товары КАК Т`
  is executed
- **THEN** each predefined item answers its declared name and every other
  row answers an empty string, exactly as on the platform

#### Scenario: Filtering by the name
- **WHEN** the field is compared with a declared name in `ГДЕ`
- **THEN** only the item declared under that name answers

#### Scenario: Through a reference
- **WHEN** the field is read as `Т.Поставщик.ИмяПредопределенныхДанных`
- **THEN** it answers from the joined table, and `NULL` where the
  reference is empty

#### Scenario: Object without predefined data
- **WHEN** the field is read from a document
- **THEN** the compiler reports an unknown field, as the platform does
