## MODIFIED Requirements

### Requirement: A reference pair used as a value
A field stored as an `RTRef`/`RRRef` pair — a register recorder, a
document journal reference — SHALL be accepted wherever a value is
expected and SHALL render as its `RTRef ‖ RRRef` payload, carrying the
runtime-typed reference kind. An aggregate over such a field SHALL be
taken over the payload and SHALL carry the same runtime-typed kind, so
the aggregated column needs no widening; an aggregate over one member
keeps that member's kind. `ТИПЗНАЧЕНИЯ` of such a field SHALL answer
the reference tag beside the table number of the row.

#### Scenario: The type of a recorder
- **WHEN** `ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Р.Регистратор) ИЗ РегистрНакопления.X КАК Р`
  is compiled
- **THEN** the column is the reference tag concatenated with the `RTRef`
  member

#### Scenario: An aggregate over a recorder
- **WHEN** `ВЫБРАТЬ МАКСИМУМ(Р.Регистратор) ИЗ РегистрНакопления.X КАК Р`
  is compiled
- **THEN** the aggregate is taken over the payload of the pair and the
  column kind is the runtime-typed reference

#### Scenario: The aggregate coalesced
- **WHEN** `ВЫБРАТЬ ЕСТЬNULL(МАКСИМУМ(Р.Регистратор), НЕОПРЕДЕЛЕНО) ИЗ РегистрНакопления.X КАК Р`
  is compiled
- **THEN** the `COALESCE` wraps the aggregate as it is
