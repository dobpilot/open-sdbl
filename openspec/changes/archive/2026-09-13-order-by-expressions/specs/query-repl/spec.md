## ADDED Requirements

### Requirement: Order by computed expressions
`УПОРЯДОЧИТЬ ПО` / `ORDER BY` SHALL accept any supported expression as an
ordering key, alongside the field path and projection alias forms. A
branch that orders by physical expressions SHALL render the compiled
expression in `ORDER BY`, keeping the written order of the keys and the
`ВОЗР`/`УБЫВ` direction. A branch whose ordering must name projected
columns — a joined branch, a grouped branch, or a union — SHALL refuse an
expression key with the diagnostic it already reports for a field that is
not projected. A statement with `ИТОГИ` SHALL project an expression key
as a hidden column so the totals wrapper can order by it. Ordering by a
type value SHALL follow the encoding, which puts the undefined type
first, then the primitive types, then references by table number, as the
platform orders them.

#### Scenario: Conditional ordering
- **WHEN** `… УПОРЯДОЧИТЬ ПО ВЫБОР КОГДА Цена > 10 ТОГДА 0 ИНАЧЕ 1 КОНЕЦ, Имя`
  is compiled for a single-source branch
- **THEN** generated SQL orders by that `CASE` and then by the name

#### Scenario: Ordering by a type
- **WHEN** `… УПОРЯДОЧИТЬ ПО ТИПЗНАЧЕНИЯ(Т.Объект), Имя` is executed over
  a composite attribute
- **THEN** the rows come out grouped by type in the platform's order

#### Scenario: Expression key in a joined branch
- **WHEN** an expression key is used in a branch with a join
- **THEN** compilation fails with `UnsupportedFeature` and the message
  that the ordering key must occur in the projection
