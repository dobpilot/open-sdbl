## MODIFIED Requirements

### Requirement: Acquire a whole configuration in one read
The database package SHALL offer one session operation answering the
resolved metadata snapshot, the resolution report, the storage layout,
every resource of the `Config` table, and the resources of each
configuration extension. Every one of those SHALL be read inside the one
read-only transaction the metadata acquisition opens, through the same
`MetadataSource`, the same acquisition pipeline, the same part assembly
and the same decoding the existing read uses. There SHALL NOT be a second
metadata pipeline nor a second path to the database.

The resources the operation answers SHALL be the very resources metadata
resolution decoded. An implementation SHALL NOT resolve metadata, end the
transaction and then read the configuration again.

The operation SHALL NOT claim that what it answers is one state of the
base. The transaction is `READ COMMITTED`, under which each statement
sees the rows committed when it began, so a base written to during the
acquisition can answer one statement from before that write and the next
from after it. The operation removes the *second* read a consumer would
otherwise perform after the session ended — it does not make the reads
atomic. The documentation SHALL say so. It SHALL restrict CONFIG_FINGERPRINT
to Config rows with a bare GUID or a .1c/.9/.7 suffix. Other Config resources
and ConfigCAS/_ExtensionsInfo/_ExtensionsRestruct SHALL be documented as
excluded. Equal fingerprints SHALL NOT be presented as proof that a complete
or selected configuration acquisition, extensions or resolved snapshot
remained unchanged.

The result SHALL be answered only after the whole acquisition succeeds.
Any failure SHALL go through the rollback the provider already performs
and SHALL answer that error; no partial result SHALL be answered.

Resource bytes SHALL be answered as the base stores them — compressed,
unmodified — and the type answering them SHALL document that. Bytes read
once SHALL be shared rather than copied per holder, so that a resource
two extensions both name is held once.

The operation, the type it answers, and the type of one acquired
extension SHALL be re-exported from the package root.

#### Scenario: Metadata and resources come from one read
- **WHEN** the operation runs against a base
- **THEN** it answers the same snapshot and the same report the metadata
  read answers, together with the resources those were resolved from,
  without reading `Config` a second time

#### Scenario: A base written to during the read
- **WHEN** a consumer needs to know whether the configuration moved while
  it was being read
- **THEN** the documentation explains READ COMMITTED and the limited
  metadata-Config fingerprint boundary, without promising whole-acquisition
  freshness or atomicity

#### Scenario: A resource the metadata read ignores
- **WHEN** the `Config` table carries a resource the acquisition filter
  rejects
- **THEN** the answered resources include it, and the snapshot is
  unchanged by it

#### Scenario: A failure part way through
- **WHEN** reading one resource fails
- **THEN** the transaction is rolled back, the error is answered, and no
  configuration is answered

#### Scenario: Excluded resource or extension changes
- **WHEN** only an ineligible Config resource or extension data changes
- **THEN** the documentation permits CONFIG_FINGERPRINT to remain equal
  and does not claim the whole configuration stayed unchanged
