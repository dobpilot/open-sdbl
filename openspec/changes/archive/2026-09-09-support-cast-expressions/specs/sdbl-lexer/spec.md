## ADDED Requirements

### Requirement: Recognize the cast keyword bilingually
The lexer SHALL classify `ВЫРАЗИТЬ` and `CAST` case-insensitively as one
keyword kind whose stable display name is `CAST`, and the exhaustive keyword
table test SHALL include both spellings.

#### Scenario: Russian and English spellings
- **WHEN** input contains `Выразить(Поле КАК СТРОКА(10))` or `cast(x as string(10))`
- **THEN** the function name is a keyword token that preserves the original
  lexeme
