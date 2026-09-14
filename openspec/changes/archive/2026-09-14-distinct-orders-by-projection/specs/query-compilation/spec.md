## ADDED Requirements

### Requirement: Ordering a distinct statement
A statement with `РАЗЛИЧНЫЕ` SHALL order by its projected columns,
addressing them by position, because such a statement keeps only the
values it projects. An ordering field the projection does not carry SHALL
be refused with a diagnostic naming the reason.

#### Scenario: Ordering by a projection alias
- **WHEN** `ВЫБРАТЬ РАЗЛИЧНЫЕ Т.Наименование КАК Имя ИЗ Справочник.X КАК Т
  УПОРЯДОЧИТЬ ПО Имя` is compiled
- **THEN** the ordering addresses the projected column by its position

#### Scenario: Ordering by a field outside the projection
- **WHEN** a distinct statement orders by a field it does not project
- **THEN** the query is refused
