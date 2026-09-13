## ADDED Requirements

### Requirement: Joins nested inside a join source
A join source SHALL accept further joins written before its own `ПО`, so
that the conditions close in reverse order, and the group SHALL compile as
the flat chain of the same joins with the outer one first. Where that
rewrite would change the result — an outer `ЛЕВОЕ`, `ПРАВОЕ` or `ПОЛНОЕ`
join containing a join that is not `ЛЕВОЕ` — the compiler SHALL report an
unsupported-feature diagnostic instead.

#### Scenario: Group of left joins
- **WHEN** `A ЛЕВОЕ СОЕДИНЕНИЕ B ЛЕВОЕ СОЕДИНЕНИЕ C ПО <B‑C> ПО <A‑B>` is
  compiled
- **THEN** the SQL joins B on the second condition and C on the first,
  answering what the platform answers

#### Scenario: Inner join inside a left join
- **WHEN** the nested join is `ВНУТРЕННЕЕ`
- **THEN** the compiler reports that the grouping is not supported
