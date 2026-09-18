## ADDED Requirements

### Requirement: Joined statements order by unprojected fields
A statement with joins and neither a union, a grouping nor `РАЗЛИЧНЫЕ`
SHALL accept an `УПОРЯДОЧИТЬ ПО` field it does not project, ordering by
the field's columns; a projected field SHALL keep ordering by its
position.

#### Scenario: Unprojected field of the joined source
- **WHEN** `ВЫБРАТЬ p.Code ИЗ … КАК p ЛЕВОЕ СОЕДИНЕНИЕ … КАК o ПО … УПОРЯДОЧИТЬ ПО o.Code, p.Code УБЫВ`
  is compiled
- **THEN** the SQL orders by the joined column, then by position 1
  descending
