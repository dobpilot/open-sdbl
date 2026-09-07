## ADDED Requirements

### Requirement: Compile change-registration sources
The compiler SHALL accept the bilingual change-registration spelling
(`<ВидОбъекта>.<X>.Изменения` / `<ObjectKind>.<X>.Changes`) on a registered
object as a FROM source and generate SQL over that object's
change-registration table, projecting the exchange-plan node reference,
message number, and the object's key columns, for both supported
dialects.

#### Scenario: Selecting registered changes
- **WHEN** a query selects the changes of an object registered with an
  exchange plan
- **THEN** both dialects produce SQL over that object's
  change-registration table with node and key columns resolvable by
  name

### Requirement: Compile calculation-kind dependency sources
The compiler SHALL expose leading, base, and displaced calculation-kind
tables as tabular-section-like sources of their chart of calculation
kinds on both dialects.

#### Scenario: Leading calculation kinds
- **WHEN** a query selects from the leading-calculation-kinds table of a
  chart of calculation kinds
- **THEN** the generated SQL reads the dependency table joined to its
  owner keys

### Requirement: Compile extension-added attributes as ordinary fields
Attributes added by configuration extensions SHALL be usable wherever
base attributes are: projection, filtering, ordering, and dereference,
compiling to the extension's physical columns without dedicated syntax.

#### Scenario: Filtering by an extension attribute
- **WHEN** a query filters on an attribute that exists only in an
  extension
- **THEN** compilation succeeds on both dialects and references the
  extension table's column

### Requirement: Diagnose resolve-only service sources
Service tables resolved as metadata but without query support SHALL
produce a machine-readable diagnostic when used as a FROM source, not
silent failure or invalid SQL.

#### Scenario: Unsupported service source
- **WHEN** a query names a resolve-only service table as its source
- **THEN** compilation fails with a typed unsupported-feature diagnostic
  naming the table
