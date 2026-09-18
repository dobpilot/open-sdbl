## ADDED Requirements

### Requirement: Dereferences across targets in expressions
A value read through a reference of several types (`Регистратор.Поле`)
SHALL be usable in an expression as its value member — the `CASE` over
the targets the projection renders — and compared with a reference
constant SHALL compare that member with the constant's `RTRef ‖ RRRef`
payload when the constant's type is known, or with the constant itself
otherwise.

#### Scenario: Filter by the recorder's organisation
- **WHEN** `… ГДЕ П.Регистратор.Организация = ЗНАЧЕНИЕ(Справочник.Организации.ПустаяСсылка)` is compiled
- **THEN** the predicate compares the `CASE` over the recorder's targets
  with the payload of the empty reference
