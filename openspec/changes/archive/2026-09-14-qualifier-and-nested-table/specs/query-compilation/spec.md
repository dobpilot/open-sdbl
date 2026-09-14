## ADDED Requirements

### Requirement: An alias hides the object name
A source that declares an alias SHALL be addressed by that alias alone.
The object name SHALL qualify only a source written without an alias,
which the platform does not accept at all and the compiler keeps as a
convenience.

#### Scenario: The same catalog read twice
- **WHEN** one statement reads a catalog under an alias and a nested
  statement reads it under another
- **THEN** each qualifier names exactly one source

### Requirement: A tabular section named as a field
A name that resolves to no field but names a tabular section of the source
SHALL report that a tabular section as a nested result of the selection is
not supported.

#### Scenario: Tabular section in the selection list
- **WHEN** `ВЫБРАТЬ Т.Состав ИЗ Справочник.X КАК Т` is compiled
- **THEN** the diagnostic names the nested result instead of an unknown
  field
