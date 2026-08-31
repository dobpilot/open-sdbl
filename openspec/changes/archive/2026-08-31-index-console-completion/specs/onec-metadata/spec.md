## ADDED Requirements

### Requirement: Build an indexed queryable-field catalog
The query layer SHALL provide a snapshot-scoped catalog of queryable fields for
the snapshot's current public object, field, schema, and live-table vectors. A
catalog build SHALL index custom fields by normalized owner table and numeric
DBNames field number rather than rescan all fields for every object. The
snapshot SHALL NOT retain indexes that become stale when callers modify those
public vectors.

#### Scenario: Unique owned custom field
- **WHEN** one owner declares `Fld<number>` and its Config descriptor has a
  human name
- **THEN** catalog projection obtains that name through indexed owner and
  number identity

#### Scenario: Caller-modified snapshot
- **WHEN** a caller changes public fields, schema, or live tables after
  resolution and then rebuilds the catalog
- **THEN** the catalog reflects current vectors and preserves the existing
  source-order first-match semantics
