## ADDED Requirements

### Requirement: Correlated subquery of a predicate
A subquery written in a predicate SHALL resolve the qualifiers of the
enclosing statement's sources and render them as the outer alias, so that
it filters by the row being tested. An unqualified name SHALL keep
resolving against the subquery's own sources only, and a derived source
SHALL keep refusing an outer qualifier, because SQL evaluates it before
the outer row exists.

#### Scenario: Existence check
- **WHEN** `ГДЕ Т.Код В (ВЫБРАТЬ Л.Код ИЗ Справочник.X КАК Л ГДЕ
  Л.Дата = Т.Дата)` is compiled
- **THEN** the subquery compares with the outer alias, as on the platform

#### Scenario: Derived source stays uncorrelated
- **WHEN** an outer qualifier is used inside `ИЗ (ВЫБРАТЬ …) КАК Д`
- **THEN** the compiler reports an unknown field

### Requirement: Order of an enumeration value
`Порядок` / `Order` SHALL name the `EnumOrder` column, which the platform
answers as the zero-based declaration order of the value.

#### Scenario: Ordering by the declaration order
- **WHEN** `ВЫБРАТЬ П.Порядок ИЗ Перечисление.X КАК П` is compiled
- **THEN** the column answers 0, 1, 2 … in declaration order
