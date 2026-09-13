## ADDED Requirements

### Requirement: Recognize the scalar function names bilingually
The lexer SHALL classify the names of the scalar string and arithmetic
functions case-insensitively as keywords whose stable display names are
the English spellings, and the exhaustive keyword table test SHALL
include every spelling. The parser SHALL treat them as contextual
identifiers, so a field or alias named after a function keeps parsing.

#### Scenario: Field named after a function
- **WHEN** input contains `ВЫБРАТЬ Окр КАК Лог ИЗ …`
- **THEN** both names are read as identifiers
