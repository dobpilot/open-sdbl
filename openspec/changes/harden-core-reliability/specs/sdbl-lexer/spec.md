## MODIFIED Requirements

### Requirement: Report malformed input

The library SHALL stop at malformed strings, parameters, malformed binary
literals, or unsupported characters and return a diagnostic with its source
position and a machine-readable category.

#### Scenario: Unterminated string

- **WHEN** the input ends inside a string literal
- **THEN** tokenization returns an unterminated-string diagnostic at the
  opening quote

#### Scenario: Unexpected character

- **WHEN** the input contains a character outside the supported lexical
  subset
- **THEN** tokenization returns an unexpected-character diagnostic carrying
  that character and its one-based line and column

## ADDED Requirements

### Requirement: Iterate tokens through the standard iterator protocol
The lexer SHALL be usable as a standard iterator yielding successful tokens
or a diagnostic, and SHALL yield nothing after the first diagnostic.

#### Scenario: Iterating a source with an error
- **WHEN** a caller iterates a source whose third token is malformed
- **THEN** the iterator yields two tokens, then one diagnostic, then no
  further items

### Requirement: Recognize the full bilingual keyword table
The lexer SHALL classify every supported keyword's Russian and English
spellings case-insensitively, and the mapping SHALL be fixed by tests
covering every keyword variant in both languages.

#### Scenario: Grouping and set-operation keywords
- **WHEN** input contains `СГРУППИРОВАТЬ`, `ИМЕЮЩИЕ`, `ОБЪЕДИНИТЬ`,
  `ПОМЕСТИТЬ`, and their English spellings
- **THEN** each token receives its corresponding keyword kind while
  preserving the original lexeme
