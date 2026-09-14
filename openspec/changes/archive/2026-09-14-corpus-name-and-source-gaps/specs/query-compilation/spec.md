## ADDED Requirements

### Requirement: Tabular sections of every kind that has them
A tabular-section source SHALL be accepted for every object kind that
stores tabular sections — catalogs, documents, charts of characteristic
types, charts of accounts, charts of calculation types, business
processes, tasks and exchange plans — and refused for the kinds that have
none.

#### Scenario: Tabular section of a business process
- **WHEN** `ИЗ БизнесПроцесс.X.ТабличнаяЧасть КАК Т` is compiled
- **THEN** the source resolves to the tabular-section table of that
  business process

### Requirement: Metadata names spelled like keywords
The kind, object and value names of `ЗНАЧЕНИЕ(…)` SHALL accept a name
that the lexer reads as a keyword, because nothing but a name may appear
in those positions.

#### Scenario: Enumeration value named like a keyword
- **WHEN** `ЗНАЧЕНИЕ(Перечисление.X.НеОпределено)` is compiled
- **THEN** the value resolves like any other predefined value
