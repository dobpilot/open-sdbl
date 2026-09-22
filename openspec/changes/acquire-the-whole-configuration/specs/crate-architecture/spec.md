## ADDED Requirements

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
atomic. The documentation SHALL say so, and SHALL name the fingerprint
statement as what a consumer compares before and after when it needs to
know whether the configuration moved.

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
- **THEN** the documentation directs it to the fingerprint statement
  rather than promising an isolation the transaction does not have

#### Scenario: A resource the metadata read ignores
- **WHEN** the `Config` table carries a resource the acquisition filter
  rejects
- **THEN** the answered resources include it, and the snapshot is
  unchanged by it

#### Scenario: A failure part way through
- **WHEN** reading one resource fails
- **THEN** the transaction is rolled back, the error is answered, and no
  configuration is answered

### Requirement: Keep each acquired extension apart
Each extension the operation answers SHALL carry its identity, its name,
whether the base applies it, the order the platform applies it in, and
its own resources. Resources SHALL be attributed to an extension through
that extension's own root index, so that two extensions naming the same
resource keep two entries rather than one shared entry. The answered
order SHALL be the platform's application order, so that a consumer can
apply the active extensions in it.

#### Scenario: Two extensions
- **WHEN** a base carries two extensions
- **THEN** each is answered with its own identity, name, order and
  resource list

#### Scenario: A name both extensions use
- **WHEN** both extensions name a resource identically
- **THEN** each keeps its own entry under that name and neither list
  gains the other's

#### Scenario: An extension the base does not apply
- **WHEN** an extension is present but not active
- **THEN** it is answered with its resources and marked inactive, so that
  a consumer can skip it

#### Scenario: An extension whose record is not understood
- **WHEN** the record of an extension does not carry the applicability
  flag where it was measured
- **THEN** the extension is answered with its activity unknown rather
  than guessed, so that a consumer cannot silently skip one the base
  applies

### Requirement: Bound what a whole-configuration read retains
The limits a session works under SHALL carry a ceiling on how many
`Config` resources a whole-configuration read may retain and on how many
compressed bytes it may hold. The read SHALL check the totals the server
reports against those ceilings before it reads a row, and SHALL check
again as it assembles, because the totals are a statement of their own
and the table may have grown since. Exceeding either SHALL be a typed
error naming the ceiling, not an exhausted process.

#### Scenario: A base larger than the ceiling
- **WHEN** the totals statement reports more resources or more bytes than
  the limits allow
- **THEN** the read fails with a typed error naming the ceiling before it
  reads the first resource row

#### Scenario: A table that grew after the totals
- **WHEN** the rows read exceed the ceiling although the totals did not
- **THEN** the read fails with the same typed error instead of retaining
  them

### Requirement: Keep the metadata read cheap
The existing metadata read SHALL keep its signature, its result and its
cost. It SHALL NOT begin to answer the resources of the configuration and
SHALL NOT be required to hold them: it SHALL keep streaming the filtered
`Config` resources and discarding each after decoding. Sharing the
acquisition pipeline with the whole-configuration read SHALL NOT change
what it reads or what it retains.

#### Scenario: Reading only the metadata
- **WHEN** an application asks for the metadata of a base
- **THEN** the same statements run as before, the whole-table statement
  does not, and no resource is retained after it is decoded
