## ADDED Requirements

### Requirement: Carry nested results of tabular sections
A compiled query MAY carry nested results, one per tabular section the
statement projects. Each nested result SHALL provide a SELECT-only
statement, the columns that statement returns, the label and position the
section takes in the logical result, and the structural link between the
statements: the index of the main-result column holding the owner key and
the index of the nested-result column matching it. The link SHALL be data,
so that a consumer building its own plan never parses the generated SQL.

The main statement SHALL select the owner key, adding it as a trailing
service column when the query does not already select it, and SHALL list
every service column so a consumer can leave it out of what it shows.

The nested statement SHALL be self-contained: it SHALL filter its rows by
the owner keys of the main statement, and SHALL order them by owner and
then by line number.

#### Scenario: Document with a projected section
- **WHEN** `ВЫБРАТЬ Д.Номер, Д.Товары ИЗ Документ.Продажа КАК Д` is
  compiled
- **THEN** the compiled query carries one nested result whose statement
  selects the section's owner reference, line number and attributes, and
  whose link names the owner-key column of each statement

#### Scenario: Named nested columns
- **WHEN** the section is written as `Д.Товары.(НомерСтроки, Товар)`
- **THEN** the nested statement selects exactly those columns plus the
  owner key, as the platform does

#### Scenario: Consumer without the generated SQL
- **WHEN** a consumer plans the query itself
- **THEN** the link tells it which columns join the two results without
  reading the SQL text
