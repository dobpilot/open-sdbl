## ADDED Requirements

### Requirement: Recognize conditional and pattern keywords bilingually
The lexer SHALL classify `ЕСТЬNULL`/`ISNULL`, `ПОДОБНО`/`LIKE`, and
`СПЕЦСИМВОЛ`/`ESCAPE` case-insensitively as three keyword kinds whose stable
display names are `ISNULL`, `LIKE`, and `ESCAPE`, and the exhaustive keyword
table test SHALL include all six spellings.

#### Scenario: Mixed-script keyword
- **WHEN** input contains `ЕстьNULL(Поле, 0)` or `isnull(x, 0)`
- **THEN** the function name is one keyword token that preserves the original
  lexeme

#### Scenario: Pattern operator keywords
- **WHEN** input contains `Наименование ПОДОБНО "А%" СПЕЦСИМВОЛ "\"` or its
  English spelling
- **THEN** `ПОДОБНО`/`LIKE` and `СПЕЦСИМВОЛ`/`ESCAPE` are keyword tokens
