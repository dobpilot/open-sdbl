## MODIFIED Requirements

### Requirement: Compound accounting fields carry member labels
A compound field of an accounting virtual table or record table SHALL
label its columns as `<Имя>_TYPE`, `<Имя>_S`, `<Имя>_N`, `<Имя>_T`,
`<Имя>_L` and `<Имя>` for the reference member, so that a `UNION` branch
projecting a scalar or `НЕОПРЕДЕЛЕНО` in that position is spread over
the same members. The member SHALL be told by the requested name even
when the output label is cut to the dialect's identifier limit.

#### Scenario: Undefined against an extra dimension
- **WHEN** one branch projects `О.Субконто2` of `Обороты` and the other
  `НЕОПРЕДЕЛЕНО`
- **THEN** the union compiles with the second branch spread over the
  members of the first

#### Scenario: Undefined against a long-named composite field
- **WHEN** one branch projects `Д.СубконтоПоАмортизационнойПремии1` and
  the other `НЕОПРЕДЕЛЕНО`, the `_TYPE` label exceeding the PostgreSQL
  identifier limit
- **THEN** the union compiles with the second branch spread over both
  members
