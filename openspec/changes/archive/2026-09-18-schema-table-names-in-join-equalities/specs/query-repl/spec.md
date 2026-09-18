## ADDED Requirements

### Requirement: Fixed references of derived sources join typed references
A join equality between a column of a nested query or temporary table
that is a reference to one object and a field that is a reference of
several types SHALL compare the field's type discriminator with the
object's database type number found in SchemaStorage, whose table names
carry no leading underscore, and the identifiers.

#### Scenario: Temporary table joined to a recorder
- **WHEN** `… ИЗ (ВЫБРАТЬ Д.Ссылка КАК Ссылка ИЗ Документ.X КАК Д) КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ … КАК Р ПО Р.Регистратор = Т.Ссылка`
  is compiled
- **THEN** the `ON` compares the recorder's type column with the
  document's number and its reference column with the derived column
