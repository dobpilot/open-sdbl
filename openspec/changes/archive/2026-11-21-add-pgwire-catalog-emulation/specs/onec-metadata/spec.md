## ADDED Requirements

### Requirement: Look up loaded descriptors by metadata identity
A finalized metadata snapshot SHALL provide read-only indexed lookup of
an object's own descriptor and an attribute's descriptors. Own-descriptor
lookup SHALL retain the first matching source record; attribute lookup
SHALL preserve all matching records in source order. The indexes SHALL
belong to the loaded snapshot and SHALL perform no I/O.

#### Scenario: Repeated field-rights checks
- **WHEN** field-rights checks inspect many attributes from one snapshot
- **THEN** each lookup visits only the matching descriptor records instead
  of scanning every descriptor in the configuration

#### Scenario: Duplicate descriptor evidence
- **WHEN** more than one descriptor carries the same attribute identity
- **THEN** indexed iteration returns all those descriptors in source order
  so existing ownership and declared-type checks retain their evidence
