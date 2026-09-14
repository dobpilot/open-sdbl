## ADDED Requirements

### Requirement: The order of the selection modifiers
`РАЗРЕШЕННЫЕ`, `РАЗЛИЧНЫЕ` and `ПЕРВЫЕ <n>` SHALL be accepted after
`ВЫБРАТЬ` in any order, each at most once, and SHALL compile to what the
canonical order compiles to. A repeated modifier SHALL be refused.

#### Scenario: Distinct before allowed
- **WHEN** `ВЫБРАТЬ РАЗЛИЧНЫЕ РАЗРЕШЕННЫЕ Т.Поле ИЗ Справочник.X КАК Т`
  is compiled
- **THEN** it compiles to what `РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ` compiles to

#### Scenario: Top before distinct
- **WHEN** `ВЫБРАТЬ ПЕРВЫЕ 1 РАЗЛИЧНЫЕ Т.Поле ИЗ Справочник.X КАК Т` is
  compiled
- **THEN** it compiles to what `РАЗЛИЧНЫЕ ПЕРВЫЕ 1` compiles to

#### Scenario: A repeated modifier
- **WHEN** `ВЫБРАТЬ РАЗЛИЧНЫЕ РАЗЛИЧНЫЕ Т.Поле ИЗ Справочник.X КАК Т` is
  compiled
- **THEN** the repetition is refused
