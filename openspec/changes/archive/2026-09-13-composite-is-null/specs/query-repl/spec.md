## ADDED Requirements

### Requirement: Test compound fields for NULL
`ЕСТЬ [НЕ] NULL` / `IS [NOT] NULL` SHALL accept a compound field — a
composite attribute or a runtime-typed reference — and SHALL render the
test on one representative physical member: the `_TYPE` discriminator
when the field has one, otherwise the `RRRef` value member. A compound
field with neither member SHALL keep reporting that it can be projected
but not used in expressions.

#### Scenario: Composite attribute of a present row
- **WHEN** `ВЫБОР КОГДА Т.Объект ЕСТЬ NULL ТОГДА 1 ИНАЧЕ 0 КОНЕЦ` is
  executed over rows of the catalog itself
- **THEN** every row answers `0`, because the discriminator is always
  written

#### Scenario: Composite attribute of a missing join row
- **WHEN** the same test is applied to the composite attribute of a
  `ЛЕВОЕ СОЕДИНЕНИЕ` whose row is absent
- **THEN** the answer is `1`, and `ЕСТЬ НЕ NULL` answers `0`
