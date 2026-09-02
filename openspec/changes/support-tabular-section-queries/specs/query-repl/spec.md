## ADDED Requirements

### Requirement: Alias projected query columns
The `open-sdbl` query compiler SHALL accept an explicit `КАК`/`AS` alias after
each supported projection expression and SHALL expose that alias as the stable
logical result label in PostgreSQL and MSSQL output.

#### Scenario: Field and dereference aliases
- **WHEN** direct and one-hop reference-property projections are followed by
  explicit aliases
- **THEN** generated SQL and `CompiledQuery.columns` use the requested aliases
  instead of the underlying field-path labels

#### Scenario: Compound projection alias
- **WHEN** an explicitly aliased logical field expands into several physical
  SQL columns
- **THEN** every member receives a unique deterministic label derived from the
  requested alias and its existing compound-member suffix

#### Scenario: Missing projection alias
- **WHEN** `КАК`/`AS` is not followed by a contextual identifier
- **THEN** compilation returns a positional diagnostic and no SQL is produced

### Requirement: Query authoritative tabular sections
The `open-sdbl` query compiler SHALL accept a document or catalog source shaped
as `<kind>.<object>.<section>`. It SHALL resolve the parent object through
DBNames and Config, resolve the section by a nested Config descriptor and its
exact DBNames `VT` entry, and require the resulting
`<parent-physical-table>_VT<number>` or its exact configuration-extension
`X[digits]` variants in SchemaStorage and the live catalog.

#### Scenario: Joined document tabular section
- **WHEN** a document tabular section is joined to another metadata source by
  its `Ссылка` field, the opposing field is a compound reference, and fields
  are selected with explicit aliases
- **THEN** generated SQL reads the exact tabular-section table, uses its owner
  reference in the JOIN, compares both the reference payload and the
  authoritative target-type discriminator, and preserves the requested output
  labels

#### Scenario: Reference property from a tabular section
- **WHEN** a tabular-section field or owner reference has one authoritative
  SchemaStorage reference target and a target property is selected
- **THEN** the existing reusable one-hop LEFT JOIN resolves that property

#### Scenario: Extended tabular-section storage
- **WHEN** the canonical tabular-section table is absent and SchemaStorage plus
  the live catalog contain exact `X[digits]` variants
- **THEN** fields are resolved from an authoritative variant and generated SQL
  reads all exact variants through one deterministic relation

#### Scenario: Inline SchemaStorage declaration
- **WHEN** SchemaStorage declares a section as
  `{"VT<number>","I",0,"<parent>",...}`
- **THEN** metadata resolution exposes the canonical
  `<parent>_VT<number>` table, its declared columns, and the implied
  `<parent>_IDRRef` owner reference

#### Scenario: Standard tabular-section fields
- **WHEN** the section table declares its parent reference and numbered line
  field
- **THEN** they are queryable as `Ссылка`/`ID` and
  `НомерСтроки`/`LineNo` respectively

#### Scenario: Invalid tabular-section mapping
- **WHEN** the parent, nested descriptor, exact `VT` entry, SchemaStorage table,
  or live table is missing or ambiguous
- **THEN** compilation returns a specific diagnostic without guessing from
  similarly prefixed physical tables
