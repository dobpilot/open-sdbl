## ADDED Requirements

### Requirement: Test reference types with the REFS operator
The compiler SHALL accept `<поле> ССЫЛКА <Вид>.<Объект>` /
`<field> REFS <Kind>.<Object>` as a predicate wherever comparisons are
accepted, the operand being a direct field or a one-hop reference
property. For a composite reference field it SHALL compare the field's
type member with the target's database type number; for a runtime-typed
nested-query column it SHALL compare the first four payload bytes; for a
fixed-target field of the named table it SHALL be a constant true
predicate, which also holds for the empty reference. A fixed-target
field of another table and a non-reference operand SHALL be `Syntax`
diagnostics, and a source-free statement SHALL refuse the operator.

#### Scenario: Composite attribute
- **WHEN** `ГДЕ Т.Объект ССЫЛКА Справочник.Товары` is compiled for a
  composite attribute
- **THEN** generated SQL compares `Т._Fld<N>_RTRef` with the catalog's
  type number on both providers

#### Scenario: Fixed-target attribute
- **WHEN** `ГДЕ Т.Клиент ССЫЛКА Справочник.Клиенты` is compiled for an
  attribute typed with that catalog only
- **THEN** generated SQL contains a true predicate, and naming another
  catalog fails with a `Syntax` diagnostic

#### Scenario: Value position
- **WHEN** `ВЫБОР КОГДА Т.Объект ССЫЛКА Справочник.Товары ТОГДА 1 ИНАЧЕ 0 КОНЕЦ`
  is compiled
- **THEN** the type test is the `CASE` condition
