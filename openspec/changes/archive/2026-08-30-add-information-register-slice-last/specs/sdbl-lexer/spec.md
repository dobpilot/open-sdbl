## ADDED Requirements

### Requirement: Recognize SliceLast keywords
The lexer SHALL classify `СрезПоследних` and `SliceLast`, case-insensitively,
as the same SliceLast keyword while preserving the exact source lexeme and
span.

#### Scenario: Bilingual SliceLast spelling
- **WHEN** Russian and English SliceLast spellings are tokenized
- **THEN** both tokens have the SliceLast keyword kind and retain their original
  text

