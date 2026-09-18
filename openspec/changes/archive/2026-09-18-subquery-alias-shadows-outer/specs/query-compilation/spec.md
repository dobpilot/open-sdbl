## MODIFIED Requirements

### Requirement: Correlated subquery of a predicate
A subquery written in a predicate SHALL resolve the qualifiers of the
enclosing statement's sources and render them as the outer alias, so that
it filters by the row being tested. A qualifier naming both a source of
the subquery and one of the enclosing statement SHALL resolve to the
subquery's own source, which hides the outer one. An unqualified name
SHALL keep resolving against the subquery's own sources only, and a derived source
SHALL keep refusing an outer qualifier, because SQL evaluates it before
the outer row exists.

#### Scenario: Existence check
- **WHEN** `ГДЕ Т.Код В (ВЫБРАТЬ Л.Код ИЗ Справочник.X КАК Л ГДЕ
  Л.Дата = Т.Дата)` is compiled
- **THEN** the subquery compares with the outer alias, as on the platform

#### Scenario: Derived source stays uncorrelated
- **WHEN** an outer qualifier is used inside `ИЗ (ВЫБРАТЬ …) КАК Д`
- **THEN** the compiler reports an unknown field

#### Scenario: Inner source hides the outer one
- **WHEN** `ИЗ Справочник.X КАК Т ГДЕ Т.Код В (ВЫБРАТЬ Т.Код ИЗ Справочник.Y КАК Т ГДЕ Т.Дата > &Д)`
  is compiled
- **THEN** every `Т` inside the subquery is the subquery's own source
