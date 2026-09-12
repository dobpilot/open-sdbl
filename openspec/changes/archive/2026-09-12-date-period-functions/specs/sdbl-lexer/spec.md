## ADDED Requirements

### Requirement: Recognize period-arithmetic keywords bilingually
The lexer SHALL classify `КОНЕЦПЕРИОДА`/`ENDOFPERIOD`,
`ДОБАВИТЬКДАТЕ`/`DATEADD`, and `РАЗНОСТЬДАТ`/`DATEDIFF` case-insensitively
as three keyword kinds whose stable display names are `ENDOFPERIOD`,
`DATEADD`, and `DATEDIFF`, and the exhaustive keyword table test SHALL
include all six spellings. The parser SHALL treat them as contextual
identifiers outside a function call.

#### Scenario: Russian and English spellings
- **WHEN** input contains `КонецПериода(Дата, МЕСЯЦ)` or `dateadd(x, DAY, 1)`
- **THEN** the function name is one keyword token that preserves the
  original lexeme
