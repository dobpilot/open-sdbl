## ADDED Requirements

### Requirement: A projection that reads no field
A projection that reads no field of a source is a constant of the row set:
it SHALL be accepted in a grouped statement without appearing among the
grouping keys, and beside an aggregate in a statement without
`СГРУППИРОВАТЬ ПО`.

#### Scenario: Label beside an aggregate
- **WHEN** `ВЫБРАТЬ "Все" КАК Метка, СУММА(Т.Цена) КАК Сумма ИЗ … КАК Т`
  is compiled
- **THEN** it compiles and answers one row, as on the platform

### Requirement: String concatenation
`+` over strings SHALL compile as concatenation on both providers. An
operand of another type beside a string SHALL be refused, as the platform
refuses it.

#### Scenario: Two strings
- **WHEN** `Т.Наименование + " ("` is compiled
- **THEN** the operands are concatenated as text

#### Scenario: A number beside a string
- **WHEN** a number is added to a string
- **THEN** the compiler refuses it
