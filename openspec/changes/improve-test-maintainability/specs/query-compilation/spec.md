## ADDED Requirements

### Requirement: Exercise metadata-dependent paths in compiler fuzzing
The query compiler fuzz fixture SHALL contain enough deterministic metadata to
reach ordinary fields, reference dereferences, and tabular-section sources,
and its workspace SHALL remain buildable in continuous integration.

#### Scenario: Fuzz a metadata-dependent query
- **WHEN** arbitrary source selects a dereferenced field or a tabular section
- **THEN** the fuzz target reaches the corresponding compiler path against its
  fixed snapshot without requiring database I/O
