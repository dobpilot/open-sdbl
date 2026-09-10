## ADDED Requirements

### Requirement: Recognize temporary-table keywords bilingually
The lexer SHALL classify `ДОБАВИТЬ`/`ADD`, `УНИЧТОЖИТЬ`/`DROP`,
`ИНДЕКСИРОВАТЬ`/`INDEX`, `НАБОРАМ`/`SETS`, and `УНИКАЛЬНО`/`UNIQUE`
case-insensitively as five keyword kinds whose stable display names are
`ADD`, `DROP`, `INDEX`, `SETS`, and `UNIQUE`, and the exhaustive keyword
table test SHALL include all ten spellings. The parser SHALL treat these
keywords as identifiers outside their clauses so that fields and aliases
with those names keep resolving.

#### Scenario: Batch keywords
- **WHEN** input contains `ДОБАВИТЬ ВТ`, `УНИЧТОЖИТЬ ВТ`, and
  `ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код) УНИКАЛЬНО)` or their English spellings
- **THEN** each keyword is one token of its kind preserving the original
  lexeme

#### Scenario: Keyword used as a field name
- **WHEN** a query projects a field named `Уникально`
- **THEN** the parser resolves it as a field reference
