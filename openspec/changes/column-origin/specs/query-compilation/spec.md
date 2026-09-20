## ADDED Requirements

### Requirement: Tell which attribute a result column comes from
Every column of a compiled query, and of a nested tabular-section result,
SHALL report the metadata it was projected from when it was projected from
metadata: the object, the tabular-section name when the source is a
section of that object, the field identity, and whether the column is one
member of a field that spreads over several columns.

A column that is not a projection of a field SHALL report no origin. The
origin SHALL be unaffected by what the label becomes: an alias given in
the text, a label truncated to the provider's identifier limit, and a
label suffixed to keep it unique all leave it as it is.

The origin SHALL come from the field the compiler resolved, not from a
lookup by name, so that a name several fields answer to cannot attach the
wrong origin to a column.

#### Scenario: A field projected under an alias
- **WHEN** a statement projects `Т.ИНН КАК Х`
- **THEN** the column is labelled `Х` and its origin names the catalog and
  the `ИНН` field

#### Scenario: Every field of a source
- **WHEN** a statement projects every field of a source
- **THEN** each column carries the origin of the field it came from

#### Scenario: A tabular section
- **WHEN** a statement projects a field of a tabular section, whether as a
  source or as a nested result
- **THEN** the origin names the owning object, the section, and the field

#### Scenario: A member of a composite field
- **WHEN** a composite field spreads over several result columns
- **THEN** each of them carries the same field origin and is marked as one
  member of several

#### Scenario: Not a field
- **WHEN** a column is an expression, an aggregate, or a literal
- **THEN** it carries no origin

#### Scenario: A truncated or suffixed label
- **WHEN** two columns of one statement would take the same label, or a
  label exceeds the provider's identifier limit
- **THEN** the labels differ or are cut, and both origins still name the
  fields the columns came from

#### Scenario: A dereferenced field
- **WHEN** a statement projects `Т.Контрагент.ИНН`
- **THEN** the origin names the counterparty catalog and its `ИНН` field,
  not the document and its `Контрагент` field
