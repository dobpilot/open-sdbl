## MODIFIED Requirements

### Requirement: Recognize basic aggregates bilingually
The lexer SHALL classify `SUM`/`СУММА`, `MIN`/`МИНИМУМ`,
`MAX`/`МАКСИМУМ`, and `AVG`/`СРЕДНЕЕ` case-insensitively as their
aggregate keyword kinds while preserving original spelling and span, and
the exhaustive keyword table test SHALL include all eight spellings. The
parser SHALL treat them as contextual identifiers outside a function call.

#### Scenario: Russian and English aggregate names
- **WHEN** input contains each Russian and English aggregate spelling
- **THEN** every token has its corresponding aggregate keyword kind

#### Scenario: Average spelled as an alias
- **WHEN** input contains `СРЕДНЕЕ(Цена) КАК Среднее`
- **THEN** the second token is a keyword token accepted as the alias
