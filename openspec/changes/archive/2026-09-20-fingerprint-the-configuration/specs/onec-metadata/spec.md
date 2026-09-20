## ADDED Requirements

### Requirement: Fingerprint a configuration without reading it
Both providers SHALL offer a SELECT-only statement that answers one value
identifying the current content of the `Config` resources of the
configuration. The value SHALL be computed by the server from a hash of
each matched resource, combined in an order the server does not vary, and
SHALL NOT transfer resource content to the client. The statement SHALL
match the same resources the acquisition statements read, on both storage
layouts, and SHALL be listed among the provider's acquisition statements.

Two reads of an unchanged base SHALL answer the same value. A change to
any matched resource SHALL change it, including a rewrite that leaves the
resource the same length.

#### Scenario: Unchanged base
- **WHEN** the statement runs twice against a base nobody wrote to
- **THEN** both runs answer the same value

#### Scenario: Rewritten resource
- **WHEN** one `Config` resource is replaced by different content of the
  same length
- **THEN** the value differs from the one read before the rewrite

#### Scenario: One round trip
- **WHEN** an application asks whether a configuration changed
- **THEN** it runs one statement and reads one row, with no resource
  content on the wire

#### Scenario: Both layouts
- **WHEN** the base stores resources in parts and when it stores each in a
  single row
- **THEN** the statement runs on both and covers the same resources the
  acquisition statements read

### Requirement: Expose the fingerprint of a loaded snapshot
`SnapshotFingerprint` and `MetadataSnapshot::fingerprint` SHALL be public,
so that an application can confirm a reloaded snapshot is the one a
prepared query was compiled against. The value SHALL remain a property of
a loaded snapshot: it says nothing about the base until the base is read,
and it is not the statement above.

#### Scenario: Confirming a reload
- **WHEN** an application reloads metadata and compares the fingerprint
  with the one it held
- **THEN** equal values mean the prepared queries it holds still match
#### Scenario: Snapshot mismatch
- **WHEN** a prepared query is compiled against a snapshot with a
  different fingerprint
- **THEN** compilation fails with the snapshot-mismatch diagnostic, as it
  does today
