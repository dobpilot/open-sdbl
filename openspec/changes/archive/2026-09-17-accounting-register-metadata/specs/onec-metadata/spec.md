## ADDED Requirements

### Requirement: Preserve accounting-register field purpose
The decoder SHALL recognize the accounting-register dimension, resource
and attribute collections of a Config descriptor by their collection
GUIDs (measured on 8.3.27) and SHALL expose the purpose on the resolved
field together with the balance flag of a dimension or resource, read
from the field's collection entry. It SHALL NOT infer the purpose or the
flag from physical column names. A physical `Fld<N>Dt`/`Fld<N>Ct` column
SHALL be attributed to field `N`.

#### Scenario: Balance and non-balance resources
- **WHEN** a register declares the balance resource `Сумма` and the
  non-balance resource `СуммаВал`
- **THEN** both resolve as accounting-register resources, the first with
  the balance flag set and the second without it

#### Scenario: Unknown collection
- **WHEN** a field descriptor sits in a collection the decoder does not
  recognize
- **THEN** its purpose stays unknown rather than guessed

### Requirement: Expose the chart of accounts of a register
The decoder SHALL read the chart of accounts an accounting register is
bound to from the register's class list (the first identifier after
the register's header, measured on 8.3.27) and SHALL expose it on the
resolved object.

#### Scenario: Register bound to a chart
- **WHEN** the UNF register `Управленческий` is resolved
- **THEN** its object carries the identity of `ПланСчетов.Управленческий`
