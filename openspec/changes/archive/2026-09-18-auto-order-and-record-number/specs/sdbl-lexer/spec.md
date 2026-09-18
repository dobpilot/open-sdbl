## ADDED Requirements

### Requirement: Automatic ordering and record number keywords
The lexer SHALL recognise `АВТОУПОРЯДОЧИВАНИЕ` / `AUTOORDER` as
`Keyword::AutoOrder` and `АВТОНОМЕРЗАПИСИ` / `RECORDAUTONUMBER` as
`Keyword::RecordAutoNumber`; the keyword table holds 108 entries.

#### Scenario: Both spellings
- **WHEN** `АВТОУПОРЯДОЧИВАНИЕ` and `RECORDAUTONUMBER` are tokenized
- **THEN** each is a keyword token of the respective kind
