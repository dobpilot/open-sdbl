## MODIFIED Requirements

### Requirement: Keep a recorded corpus of real queries
The repository SHALL carry the query texts of a real 1C configuration
together with the result the compiler produces for each, and a metadata
fixture of that configuration pruned to the objects those queries reach.
A test SHALL recompile every query against the fixture and compare the
result with the recorded one, failing on any difference, and SHALL assert
the number of queries that compile.

Only text the lexer and parser accept as a query SHALL be recorded, so
that interface captions beginning with a query keyword stay out of the
corpus. Each recorded entry SHALL carry the values its parameters take, so
a query is compiled against a value of the type its use requires rather
than against an untyped `NULL`. An entry whose metadata the pruned fixture
does not carry SHALL be recorded as such, and the asserted count SHALL
cover the applicable entries only.

#### Scenario: Unchanged compiler
- **WHEN** the corpus test runs against an unchanged compiler
- **THEN** every query produces its recorded SQL or diagnostic

#### Scenario: Improvement
- **WHEN** a change makes a previously refused query compile
- **THEN** the test fails with the difference, and the recorded result and
  the count are updated in the same commit

#### Scenario: Recording a caption
- **WHEN** the corpus is re-recorded from a configuration whose texts
  include an interface caption such as `Выбрать пользователя`
- **THEN** the caption is not recorded, because no query can be parsed
  from it

#### Scenario: Query with a typed parameter
- **WHEN** a recorded query slices a register with `&ДатаОкончания`
- **THEN** the harness binds that parameter to a date and the query
  compiles, instead of being refused for an untyped parameter

#### Scenario: Entry beyond the fixture
- **WHEN** a recorded query names a metadata object the pruned fixture
  does not carry
- **THEN** the entry is reported as inapplicable rather than as a
  compiler gap, and it is excluded from the asserted count
