## ADDED Requirements

### Requirement: Resolve live metadata from precomputed membership indexes
The metadata resolver SHALL derive SchemaStorage field ownership, live field
presence, and live table identity with lookup structures built in bounded
passes over their respective inputs rather than rescanning all tables and
columns for every DBNames object or field. It SHALL preserve observable
resolution results and source order.

#### Scenario: Schema field ownership
- **WHEN** a canonical `Fld<number>` column or one of its compound members is
  declared by multiple SchemaStorage tables
- **THEN** the resolved field lists each owning table once in SchemaStorage
  source order

#### Scenario: Live compound field presence
- **WHEN** the live catalog contains an exact `_fld<number>` column or a
  compound member separated by `_`
- **THEN** the corresponding resolved field is live after canonical recasing

#### Scenario: Similar numeric prefix
- **WHEN** the catalog contains `_fld123` but DBNames contains only `Fld12`
- **THEN** the resolver does not treat the longer numeric field as a match

#### Scenario: Live table identity
- **WHEN** a DBNames object has a corresponding lowercase PostgreSQL table
- **THEN** object live-state, allowed-length inference, and standard-field
  indexing match the prior case-insensitive resolution
