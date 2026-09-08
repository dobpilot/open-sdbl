## ADDED Requirements

### Requirement: Recognize the reference UUID keyword bilingually
The lexer SHALL classify `УНИКАЛЬНЫЙИДЕНТИФИКАТОР` and `UUID`
case-insensitively as one keyword kind whose stable display name is `UUID`,
and the exhaustive keyword table test SHALL include both spellings.

#### Scenario: Russian and English spellings
- **WHEN** input contains `УникальныйИдентификатор(Ссылка)` or `uuid(Ref)`
- **THEN** the function name is a keyword token that preserves the original
  lexeme
