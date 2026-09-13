## ADDED Requirements

### Requirement: Recognize the reference test keyword bilingually
The lexer SHALL classify `ССЫЛКА` and `REFS` case-insensitively as one
keyword kind whose stable display name is `REFS`, and the exhaustive
keyword table test SHALL include both spellings. The parser SHALL treat
the keyword as a contextual identifier, so the standard field `Ссылка`
keeps parsing in every field position.

#### Scenario: Operator and field spelled the same
- **WHEN** input contains `ГДЕ Т.Ссылка ССЫЛКА Справочник.Товары`
- **THEN** the first `Ссылка` is a field segment and the second is the
  operator keyword
