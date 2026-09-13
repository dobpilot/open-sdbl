## ADDED Requirements

### Requirement: Compile the scalar string and arithmetic functions
The compiler SHALL accept `ПОДСТРОКА(x, n, m)`, `ДлинаСтроки(x)`,
`СокрЛП(x)`, `СокрЛ(x)`, `СокрП(x)`, `ВРег(x)`, `НРег(x)`, `Лев(x, n)`,
`Прав(x, n)`, `СтрНайти(x, s)`, `СтрЗаменить(x, s, r)`, `Окр(x[, n])`,
`Цел(x)`, `Sqrt`, `Exp`, `Log`, `Log10`, `Pow`, `Cos`, `Sin`, `Tan`,
`ACos`, `ASin`, and `ATan`, with their English spellings, and SHALL
render each per dialect. String functions SHALL return a string and the
others a number. An argument of the wrong kind SHALL be a `Syntax`
diagnostic, `NULL` and unclassified values passing. The arity SHALL be
checked at parse time. PostgreSQL renderings SHALL stay portable to 9.0,
and a character column of the 1C extension types SHALL be cast to `text`
before the call. `Log` SHALL be the natural logarithm, `Log10` the
decimal one, `Окр` SHALL round half away from zero and accept a negative
scale, and `Цел` SHALL truncate toward zero, as measured on the platform.

#### Scenario: String functions
- **WHEN** `ПОДСТРОКА(Т.Наименование, 2, 3)`, `Лев(Т.Наименование, 3)`,
  and `СтрНайти(Т.Наименование, "ан")` are compiled
- **THEN** PostgreSQL renders `substring(… from 2 for 3)`,
  `substring(… from 1 for 3)`, and `position('ан' in …)`, and the results
  match the platform row for row

#### Scenario: Trailing spaces
- **WHEN** `ДлинаСтроки("  а  ")` is compiled
- **THEN** the answer is 5 on both providers, so SQL Server cannot use
  `LEN` alone

#### Scenario: Wrong argument kind
- **WHEN** `ВРег(Т.Цена)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic naming the
  expected kind
