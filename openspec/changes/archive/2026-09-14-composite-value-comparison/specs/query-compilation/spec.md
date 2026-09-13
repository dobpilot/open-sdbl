## ADDED Requirements

### Requirement: Compare a composite field with a value
A field stored in several physical members SHALL be comparable with a
value by `=`, `<>` and `В (…)`. The rendered predicate SHALL test the
`_TYPE` discriminator against the type tag of the value and the member
that carries a value of that type, which is how the platform renders its
own comparison. A reference value SHALL also test the `RTRef` member, and
where the field admits a single reference type and stores no `RTRef`
member the comparison SHALL synthesize it from the discriminator. A value
whose type the field cannot hold SHALL compare false instead of failing,
and an unbound parameter SHALL render as a comparison with `NULL`.

#### Scenario: Reference value
- **WHEN** `ГДЕ Т.Объект = &Ссылка` compares a composite field with a
  bound catalog reference
- **THEN** only rows holding that reference answer, as on the platform

#### Scenario: Value of another stored type
- **WHEN** the same field is compared with a string
- **THEN** only rows holding that string answer

#### Scenario: List of values
- **WHEN** `ГДЕ Т.Объект В (НЕОПРЕДЕЛЕНО, ЗНАЧЕНИЕ(…), "текст")` is
  compiled
- **THEN** the predicate is the disjunction of the member comparisons of
  the listed values

#### Scenario: Value of an impossible type
- **WHEN** a composite field that holds no number is compared with one
- **THEN** the comparison answers no rows instead of reporting a
  diagnostic
