## ADDED Requirements

### Requirement: Group register virtual tables by the dimensions in use
`Остатки` and `Обороты` SHALL answer one row per combination of the
dimensions the statement resolves against the source, summing their
resources over every other dimension, as the platform does. A statement
that resolves no dimension SHALL get exactly one row. A dimension named
only inside the virtual table's own condition SHALL not add a grouping
level. Every virtual table SHALL also be accepted without its argument
list, which is how the platform writes it when no argument is given.

#### Scenario: One dimension of two
- **WHEN** `ВЫБРАТЬ О.Товар, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты КАК О`
  is executed over a register with dimensions `Товар` and `Клиент`
- **THEN** there is one row per товар, its turnover summed over клиенты

#### Scenario: No dimension
- **WHEN** only a resource is selected
- **THEN** there is exactly one row holding the turnover of the register

#### Scenario: Condition on an unread dimension
- **WHEN** the virtual table's condition filters by `Клиент` and the
  statement selects only the resource
- **THEN** there is one row, filtered but not grouped
