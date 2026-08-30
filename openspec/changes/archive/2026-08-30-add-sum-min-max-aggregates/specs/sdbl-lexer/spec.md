## ADDED Requirements

### Requirement: Recognize basic aggregates bilingually
The lexer SHALL classify `SUM`/`СУММА`, `MIN`/`МИНИМУМ`, and
`MAX`/`МАКСИМУМ` case-insensitively as their aggregate keyword kinds while
preserving original spelling and span.

#### Scenario: Russian and English aggregate names
- **WHEN** input contains each Russian and English aggregate spelling
- **THEN** every token has its corresponding aggregate keyword kind
