## ADDED Requirements

### Requirement: Read a filter criterion
`КритерийОтбора.<Имя>(<значение>)` / `FilterCriterion` SHALL be a source
that answers every object whose field listed in the criterion's content
holds the value. It SHALL compile as one `SELECT` per content field united
by `UNION ALL`, each projecting the found object as the `RTRef ‖ RRRef`
payload of the single field `Ссылка`, so that the field dereferences,
groups and joins like a reference of a derived source. A reference value
SHALL be compared by its 16-byte identifier. A criterion whose content
reaches no live field SHALL be refused.

#### Scenario: Objects found by a criterion
- **WHEN** `ВЫБРАТЬ К.Ссылка ИЗ КритерийОтбора.X(&Значение) КАК К` is
  compiled
- **THEN** the relation unites one selection per content field, each
  filtered by the value

#### Scenario: Dereference of the found object
- **WHEN** `К.Ссылка.Наименование` is read
- **THEN** each target is joined under its own type guard, as for any
  payload reference
