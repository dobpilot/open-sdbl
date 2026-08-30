## ADDED Requirements

### Requirement: Recognize COUNT bilingually
The lexer SHALL classify `COUNT` and `КОЛИЧЕСТВО` case-insensitively as one
aggregate keyword while preserving the original lexeme and span.

#### Scenario: English and Russian aggregate names
- **WHEN** input contains `count` or `Количество`
- **THEN** both tokens have the COUNT keyword kind and retain their spelling
