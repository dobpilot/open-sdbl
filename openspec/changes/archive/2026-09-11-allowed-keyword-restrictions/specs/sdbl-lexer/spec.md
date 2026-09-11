## ADDED Requirements

### Requirement: Recognize the allowed keyword bilingually
The lexer SHALL classify `РАЗРЕШЕННЫЕ` and `ALLOWED` case-insensitively as
one keyword kind whose stable display name is `ALLOWED`, and the exhaustive
keyword table test SHALL include both spellings.

#### Scenario: Allowed keyword
- **WHEN** input contains `ВЫБРАТЬ РАЗРЕШЕННЫЕ` or `SELECT allowed`
- **THEN** the second token is the `ALLOWED` keyword preserving the original
  lexeme
