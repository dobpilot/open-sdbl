## ADDED Requirements

### Requirement: Recognize SliceFirst keywords
The lexer SHALL classify `СрезПервых` and `SliceFirst`, case-insensitively,
as the same SliceFirst keyword while preserving exact source spelling and span.

#### Scenario: Bilingual SliceFirst spelling
- **WHEN** Russian and English SliceFirst spellings are tokenized
- **THEN** both tokens have the SliceFirst keyword kind and retain their
  original text

