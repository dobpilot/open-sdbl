## ADDED Requirements

### Requirement: Compile IN-list predicates

The compiler SHALL accept bilingual `В`/`IN` after a scalar expression and a
non-empty parenthesized, comma-separated list of scalar expressions. It SHALL
preserve list order, compile each member using the left operand's type context,
and emit an SQL `IN (...)` predicate for PostgreSQL and MSSQL.

#### Scenario: Several predefined catalog values
- **WHEN** a predicate uses
  `В (ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.Утвержден), ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.ДополнительныеУсловияПоДоговору_Проверен))`
- **THEN** generated SQL compares the left operand with both resolved catalog
  `_IDRRef` lookup expressions in the same order

#### Scenario: English alias and typed literals
- **WHEN** a predicate uses `IN` with one or more scalar literals
- **THEN** each literal uses the target dialect and the left field's resolved
  physical type

#### Scenario: Empty or malformed list
- **WHEN** `В`/`IN` is followed by an empty list, a trailing comma, or no closing
  parenthesis
- **THEN** compilation returns a positional diagnostic and emits no SQL
