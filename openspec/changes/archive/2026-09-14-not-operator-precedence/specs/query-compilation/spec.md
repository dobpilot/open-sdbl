## ADDED Requirements

### Requirement: Precedence of the negation operator
`НЕ` / `NOT` SHALL bind looser than every comparison and tighter than `И`,
so that it negates the comparison, `ПОДОБНО`, `В`, `В ИЕРАРХИИ`, `ССЫЛКА`
or `ЕСТЬ NULL` written to its right, and groups before a conjunction. The
unary sign operators keep binding to the operand next to them.

#### Scenario: Negated comparison
- **WHEN** `ГДЕ НЕ Т.Цена = 10` is compiled
- **THEN** the predicate negates the comparison, answering every row whose
  price differs from ten, as the platform does

#### Scenario: Negation before a conjunction
- **WHEN** `ГДЕ НЕ Т.Цена > 10 И Т.Цена < 30` is compiled
- **THEN** the negation covers only the first comparison
