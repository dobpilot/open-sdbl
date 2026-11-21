## ADDED Requirements

### Requirement: Authorize a tabular section's owner reference
For a verified document or catalog tabular section, the field checker SHALL
map its standard reference to the existing owner-reference Read check.
The section SHALL belong to the owner's complete Config section collection
and have an unambiguous descriptor and DBNames VT entry. Loaded-role and
role-union checks, explicit section Read refusals, and query RLS SHALL
remain effective.

#### Scenario: Read the invoice reference from its rows
- **WHEN** a user with document Read selects `УникальныйИдентификатор(Ссылка)`
  and `ПредставлениеСсылки(Ссылка)` from `Документ.ПриходнаяНакладная.Запасы`
- **THEN** the verified owner reference passes field authorization and
  the query executes through normal compilation and RLS

#### Scenario: Owner or section Read is refused
- **WHEN** no loaded role grants the owner reference and permits the section
- **THEN** the query fails with permission denied before database execution

#### Scenario: Unknown section or unsupported field
- **WHEN** the section is unresolved, foreign or ambiguous, or the field
  is not a verified owner reference
- **THEN** the checker does not authorize it through the owner-reference rule

### Requirement: Resolve custom fields within their tabular section
For a verified document or catalog section, custom field ownership SHALL
use the section's complete Config attribute collection and the loaded field
GUID and declaration, rather than the owner's flat field-name lookup.
Foreign fields, incomplete collections and ambiguous declarations SHALL
fail closed. Names repeated in other sections SHALL not change ownership.

#### Scenario: Read an invoice item
- **WHEN** a query reads `Номенклатура` from `Документ.ПриходнаяНакладная.Запасы`
- **THEN** its GUID is verified against `Запасы` and the query reaches the
  ordinary per-user rights and RLS checks

#### Scenario: A field from a neighboring section is forged
- **WHEN** a field GUID is absent from the selected section's known collection
- **THEN** the request is refused even when that field belongs to the same document

### Requirement: Authorize verified tabular section fields
Verified custom fields and the standard row number SHALL use owner Read
with complete role loading and role union. Explicit section and custom-field Read
refusals SHALL be honored for direct GUID entries and exact member paths.
Ambiguous paths and unsupported child restrictions SHALL fail closed.
UI rights and attribute defaults SHALL not replace owner Read. Existing
query RLS enforcement SHALL remain in effect.

#### Scenario: Read known section fields
- **WHEN** a loaded role grants owner Read and does not refuse the section
  or its verified fields
- **THEN** reference, row number and known custom fields pass field checks

#### Scenario: A section attribute is refused
- **WHEN** the selected roles do not jointly provide a complete owner,
  section and field Read grant
- **THEN** projection, filtering and sorting on that attribute fail before SQL
