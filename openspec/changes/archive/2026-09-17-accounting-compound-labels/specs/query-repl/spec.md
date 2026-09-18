## ADDED Requirements

### Requirement: Compound accounting fields carry member labels
A compound field of an accounting virtual table or record table SHALL
label its columns as `<Имя>_TYPE`, `<Имя>_S`, `<Имя>_N`, `<Имя>_T`,
`<Имя>_L` and `<Имя>` for the reference member, so that a `UNION` branch
projecting a scalar or `НЕОПРЕДЕЛЕНО` in that position is spread over
the same members.

#### Scenario: Undefined against an extra dimension
- **WHEN** one branch projects `О.Субконто2` of `Обороты` and the other
  `НЕОПРЕДЕЛЕНО`
- **THEN** the union compiles with the second branch spread over the
  members of the first
