## ADDED Requirements

### Requirement: Recognize the range keyword bilingually
The lexer SHALL classify `МЕЖДУ` and `BETWEEN` case-insensitively as one
keyword kind whose stable display name is `BETWEEN`, and the exhaustive
keyword table test SHALL include both spellings. The parser SHALL treat
the keyword as a contextual identifier.

#### Scenario: Range predicate
- **WHEN** input contains `ГДЕ Т.Цена МЕЖДУ 8 И 22`
- **THEN** `МЕЖДУ` is the keyword and `И` keeps being the conjunction
