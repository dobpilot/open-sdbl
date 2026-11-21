## ADDED Requirements

### Requirement: Name the standard reference in Russian projections
Queryable metadata and wildcard projections SHALL name the standard object
and tabular-section owner reference `Ссылка`, retaining `ID` as a query
alias and canonical storage identity. Custom fields named ID and explicit
result aliases SHALL retain their names. Standard-field rights identities
and physical SQL references SHALL remain unchanged.

#### Scenario: Wildcard reference
- **WHEN** a query or catalog enumerates fields of an object or section
- **THEN** the standard reference is named Ссылка and retains StandardFieldId::Id

#### Scenario: Explicit result alias
- **WHEN** a user selects Ссылка AS ID
- **THEN** the resulting column is named ID
