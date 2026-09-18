## ADDED Requirements

### Requirement: Order by a compound field
An `УПОРЯДОЧИТЬ ПО` term naming a field of several columns — by path or
by the alias of its projection — SHALL order by each column in the
field's column order, every column with the term's direction.

#### Scenario: Recorder and extra dimension
- **WHEN** `… УПОРЯДОЧИТЬ ПО Д.Регистратор, Д.СубконтоДт1 УБЫВ` is compiled
- **THEN** the `ORDER BY` lists the recorder's type and reference columns
  ascending, then the extra dimension's type and reference columns
  descending
