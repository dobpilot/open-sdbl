## ADDED Requirements

### Requirement: Shared names of a derived source fall back to labels
When two or more columns of a nested query or temporary table carry the
same name, each SHALL be addressable by its emitted label — the first
under the name itself, the next under the allocator's suffixed label —
instead of an `AmbiguousField` diagnostic.

#### Scenario: Two unaliased fields of one name
- **WHEN** a nested query projects `Д.Организация, Д.ПодразделениеДт КАК Организация`
  and the outer statement reads `Организация` and `Организация_2`
- **THEN** both resolve to their columns
