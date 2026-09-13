## ADDED Requirements

### Requirement: Compile range predicates
The compiler SHALL accept `<выражение> [НЕ] МЕЖДУ <нижняя> И <верхняя>` /
`<expression> [NOT] BETWEEN <lower> AND <upper>` wherever comparisons are
accepted, with any supported scalar expressions as the value and the
bounds, and SHALL render `BETWEEN` / `NOT BETWEEN`. The bounds SHALL be
inclusive, reversed bounds SHALL select no rows, and a `NULL` value SHALL
not match, as on the platform.

#### Scenario: Numeric range
- **WHEN** `ГДЕ Т.Цена МЕЖДУ 8 И 22` is compiled
- **THEN** generated SQL is `(… BETWEEN 8 AND 22)` and the rows with the
  bound values are selected

#### Scenario: Negated range over expressions
- **WHEN** `ГДЕ Т.Цена НЕ МЕЖДУ Т.Цена - 1 И Т.Цена + 1` is compiled
- **THEN** generated SQL is `(NOT (… BETWEEN … AND …))`
