## ADDED Requirements

### Requirement: Build the completion catalog from indexed metadata
The interactive console SHALL project queryable fields at most once per live
named object during each completion-catalog rebuild, resolve reference targets
through indexed physical-table identity, and deduplicate candidates through
normalized membership rather than repeated full candidate scans. It SHALL
preserve the existing commands, keywords, object spellings, field aliases,
qualified fields, reference paths, case-insensitive uniqueness, and sorted
completion behavior.

#### Scenario: Duplicate spelling with different case
- **WHEN** multiple metadata sources contribute candidates differing only by
  case
- **THEN** only the first spelling is retained, as before

#### Scenario: Qualified field completion
- **WHEN** a resolved object exposes a queryable field alias
- **THEN** completion includes the same bare and object-qualified candidates

#### Scenario: Reference path completion
- **WHEN** a queryable reference field has a uniquely resolved target object
- **THEN** completion includes the same source-alias and target-alias paths
  without reprojecting the target for each reference field
