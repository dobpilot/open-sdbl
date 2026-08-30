## ADDED Requirements

### Requirement: Recognize accumulation virtual-table keywords
The lexer SHALL classify `Остатки`/`Balance` and `Обороты`/`Turnovers`
case-insensitively as their respective keyword kinds while preserving spelling
and span.

#### Scenario: Bilingual accumulation virtual tables
- **WHEN** Russian and English Balance and Turnovers spellings are tokenized
- **THEN** every token has its corresponding keyword kind and original text

