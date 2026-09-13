## ADDED Requirements

### Requirement: Recognize the balance-and-turnovers keyword
The lexer SHALL classify `ОСТАТКИИОБОРОТЫ` and `BALANCEANDTURNOVERS`
case-insensitively as one keyword kind whose stable display name is
`BALANCEANDTURNOVERS`, and the exhaustive keyword table test SHALL
include both spellings. The parser SHALL treat it as a contextual
identifier.

#### Scenario: Virtual table name
- **WHEN** input contains `ИЗ РегистрНакопления.Продажи.ОстаткиИОбороты КАК О`
- **THEN** the name is the virtual table keyword
