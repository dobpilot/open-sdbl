## ADDED Requirements

### Requirement: Recognize totals keywords bilingually
The lexer SHALL classify `ИТОГИ`/`TOTALS`, `ОБЩИЕ`/`OVERALL`,
`ИЕРАРХИЯ`/`HIERARCHY`, `ТОЛЬКО`/`ONLY`, and `ПЕРИОДАМИ`/`PERIODS`
case-insensitively as five keyword kinds whose stable display names are
the English spellings, and the exhaustive keyword table test SHALL
include all ten spellings. The parser SHALL treat them as contextual
identifiers outside the totals clause.

#### Scenario: Totals clause
- **WHEN** input contains `ИТОГИ СУММА(Сумма) ПО ОБЩИЕ, Товар ИЕРАРХИЯ`
- **THEN** `ИТОГИ`, `ОБЩИЕ`, and `ИЕРАРХИЯ` are keyword tokens that
  preserve their lexemes
