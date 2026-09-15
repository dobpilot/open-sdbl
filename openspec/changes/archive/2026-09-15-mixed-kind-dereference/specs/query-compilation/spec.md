## ADDED Requirements

### Requirement: A dereference across targets of different types
A dereference whose targets type the field differently SHALL answer one
composite value: every target writes the member of its own type, the zero
of every other member and the tag of its own type, and each member stays
`NULL` while that target's value is `NULL`. The members SHALL be selected
by the stored type, the way the targets themselves are. A value that has no
place in a composite SHALL keep the mismatch diagnostic.

#### Scenario: One target keeps a string where another keeps a reference
- **WHEN** `ВЫБРАТЬ Р.Регистратор.Поле ИЗ РегистрНакопления.X КАК Р` is
  compiled and the recorders type `Поле` differently
- **THEN** the result carries the members of a composite value, the
  discriminator among them
