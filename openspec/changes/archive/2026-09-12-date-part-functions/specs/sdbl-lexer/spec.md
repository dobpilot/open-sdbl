## ADDED Requirements

### Requirement: Recognize date-part keywords bilingually
The lexer SHALL classify `ГОД`/`YEAR`, `КВАРТАЛ`/`QUARTER`,
`МЕСЯЦ`/`MONTH`, `ДЕНЬГОДА`/`DAYOFYEAR`, `ДЕНЬ`/`DAY`, `НЕДЕЛЯ`/`WEEK`,
`ДЕНЬНЕДЕЛИ`/`WEEKDAY`, `ЧАС`/`HOUR`, `МИНУТА`/`MINUTE`, and
`СЕКУНДА`/`SECOND` case-insensitively as ten keyword kinds whose stable
display names are the English spellings, and the exhaustive keyword table
test SHALL include all twenty spellings. The parser SHALL treat them as
contextual identifiers outside a function call, so period names, aliases,
and field names spelled the same keep parsing.

#### Scenario: Function and alias spelled the same
- **WHEN** input contains `ГОД(Дата) КАК Год`
- **THEN** both `ГОД` and `Год` are keyword tokens and the query compiles
  with the alias `Год`

#### Scenario: Period name after the keyword change
- **WHEN** input contains `НАЧАЛОПЕРИОДА(Дата, ДЕНЬ)`
- **THEN** `ДЕНЬ` is accepted as the period identifier
