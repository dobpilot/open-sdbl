## ADDED Requirements

### Requirement: Project every declared inline table kind
The SchemaStorage projection SHALL recognize all inline table kinds
declared by the platform — tabular sections, accounting extra
dimensions, calculation-kind dependency tables, change-registration
tables, and recalculation tables — including their owner linkage and
synthesized owner-reference columns. Inline declarations of an unknown
kind SHALL still be isolated and reported, never silently dropped.

#### Scenario: Extra-dimension declaration
- **WHEN** a chart of accounts declares extra-dimension tables
- **THEN** the projection yields tables linked to their owning chart
  with their key columns intact

#### Scenario: Unknown inline kind
- **WHEN** a declaration uses an inline kind the library does not know
- **THEN** the table is recorded as a schema anomaly and the containing
  object remains resolved

### Requirement: Resolve platform service tables as typed metadata
Metadata resolution SHALL resolve change-registration, recalculation,
calculation-kind dependency, and extra-dimension tables to typed
metadata objects owned by their registered object, chart, or register,
using DBNames as the naming authority.

#### Scenario: Change-registration ownership
- **WHEN** a resolved base contains change-registration tables
- **THEN** each resolves to a typed object linked to the object whose
  changes it registers, with queryable node and key columns

#### Scenario: Service mapping mismatch
- **WHEN** a non-zero service DBNames entry has no matching
  SchemaStorage declaration by alias and number
- **THEN** resolution records a typed finding instead of silently
  dropping the entry

### Requirement: Merge extension declarations into owning objects
Given the extension side of Config and DBNames, resolution SHALL merge
extension-added attributes into the owning object's queryable fields,
mapped to the extension's physical columns, and SHALL record their
extension origin. Extension tables that mirror a declared base object
SHALL count as declared.

#### Scenario: Extension-added attribute
- **WHEN** an extension adds an attribute to a configuration object
- **THEN** the resolved object exposes that attribute as a queryable
  field mapped to the extension table's column

#### Scenario: Extension field-number collision
- **WHEN** an extension maps an existing base `Fld` number to a
  different GUID
- **THEN** the base mapping is retained, the conflicting extension
  field is rejected, and resolution records a typed finding

#### Scenario: Report silence for supported families
- **WHEN** a base with extensions and service tables is resolved with
  extension inputs supplied
- **THEN** the resolution report lists only genuinely unknown tables

### Requirement: Parse reference dumps without spurious findings
Decoding and resolution SHALL be exercised against whole `DBNames` and
`SchemaStorage` dumps captured from reference bases, and SHALL neither
fail nor emit findings that do not correspond to a real mismatch.

#### Scenario: Shared and zero GUID entries
- **WHEN** a real `DBNames` resource contains many entries that share an
  owner GUID and many entries carrying the all-zero GUID
- **THEN** decoding succeeds, the all-zero entries resolve to no object,
  and no duplicate-GUID finding is produced for either group

#### Scenario: Complete column-tag coverage
- **WHEN** a real `SchemaStorage` resource is projected
- **THEN** every declared column type tag is recognized and no
  unknown-column-tag or invalid-declaration finding is produced

#### Scenario: Cross-provider equivalence
- **WHEN** the same logical configuration is resolved from a PostgreSQL
  dump and from an MSSQL dump
- **THEN** shared objects resolve to equivalent canonical names,
  physical names, and field sets
