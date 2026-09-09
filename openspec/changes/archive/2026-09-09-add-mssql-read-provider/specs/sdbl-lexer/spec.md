## ADDED Requirements

### Requirement: Recognize hexadecimal binary literals
The lexer SHALL return `0x`/`0X` followed by a non-empty even number of ASCII
hexadecimal digits as one binary-literal token while preserving its original
spelling and source span. Malformed binary literals SHALL produce a positional
diagnostic instead of being split into number and identifier tokens.

#### Scenario: Rowversion literal
- **WHEN** a query contains `0x00000000000007D6`
- **THEN** the lexer returns one binary-literal token containing the complete
  spelling

#### Scenario: Malformed binary literal
- **WHEN** a `0x` literal is empty, has an odd digit count, or contains a
  non-hexadecimal identifier character
- **THEN** tokenization returns an invalid-binary-literal diagnostic at the
  `0x` prefix
