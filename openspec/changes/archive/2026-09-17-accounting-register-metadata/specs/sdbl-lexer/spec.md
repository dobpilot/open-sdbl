## ADDED Requirements

### Requirement: Recognize accounting virtual-table keywords
The lexer SHALL classify `ОБОРОТЫДТКТ`/`DRCRTURNOVERS` and
`ДВИЖЕНИЯССУБКОНТО`/`RECORDSWITHEXTDIMENSIONS` as keywords that stay
contextual identifiers, so a field or alias spelled the same way keeps
parsing.

#### Scenario: Keyword after a register name
- **WHEN** `РегистрБухгалтерии.Управленческий.ОборотыДтКт(` is tokenized
- **THEN** `ОборотыДтКт` is a keyword token
