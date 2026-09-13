## ADDED Requirements

### Requirement: Dereference standard fields through a composite reference
A dereference through a reference that admits several tables SHALL
resolve standard fields as well as attributes. SchemaStorage names no
target for such a reference, so the candidates are scanned from the
snapshot; the scan SHALL treat every accepted spelling of a standard
field as present in every reference object and leave the candidate limit
and its `ВЫРАЗИТЬ` advice unchanged.

#### Scenario: Recorder date
- **WHEN** `ВЫБРАТЬ О.Регистратор.Дата ИЗ РегистрНакопления.Продажи КАК О`
  is executed
- **THEN** the answer is the date of the document that wrote each record

#### Scenario: Parent of a composite attribute
- **WHEN** `ВЫБРАТЬ Т.Объект.Родитель ИЗ Справочник.Товары КАК Т` is
  executed over an attribute holding references to two catalogs
- **THEN** the rows whose value is a catalog with a parent answer it, and
  the others answer `NULL`
