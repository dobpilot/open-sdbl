## ADDED Requirements

### Requirement: Acquire every resource of the Config table
Both providers SHALL offer SELECT-only statements reading every row of
the `Config` table as `(name, part, data)`, ordered by name and part, in
a variant for each storage layout: the multi-part variant orders by
`PartNo` and returns it, and the legacy variant reports part zero for the
single row a resource has. The statements SHALL carry no name filter, so
that a resource the acquisition filter rejects — a name that is not a
bare GUID or a GUID with `.1c`, `.9` or `.7` — is read like any other.
Each provider SHALL also offer the matching totals statement counting the
distinct resources and the compressed bytes of every part, and both
statements SHALL be listed among the provider's acquisition statements.

Parts SHALL be assembled by the rule that already governs multi-part
resources: contiguous from zero, concatenated in ascending part order,
and a gap, a repeat or a start after zero SHALL be a data error naming
the resource.

#### Scenario: A resource outside the acquisition filter
- **WHEN** the table carries a resource whose name is not GUID-shaped
- **THEN** the whole-table statement returns it and the filtered
  acquisition statement does not

#### Scenario: A resource stored in parts
- **WHEN** a resource is stored as parts 0, 1 and 2
- **THEN** the whole-table read answers one resource whose bytes are the
  three parts concatenated in order, and answers it once

#### Scenario: A broken part sequence
- **WHEN** a resource has parts 0 and 2, or part 0 twice, or starts at
  part 1
- **THEN** the read fails with a data error naming that resource and the
  part that was expected

#### Scenario: Legacy layout
- **WHEN** the base has no `PartNo` column
- **THEN** the legacy variant reads each resource as one row reported as
  part zero, and the result is the same shape as on a modern base

### Requirement: Name the Config resources metadata resolution decodes
The core SHALL expose one predicate answering whether a `Config` resource
name is one metadata resolution decodes: a bare GUID, or a GUID followed
by `.1c`, `.9` or `.7`. The predicate SHALL match exactly the names the
filtered acquisition statements of both providers select, so that a read
of the whole table and a filtered read decode the same set of resources.

A name the predicate rejects SHALL NOT be handed to the resource decoder,
and SHALL still be counted towards the progress a read reports.

#### Scenario: The shapes the statements match
- **WHEN** the predicate is asked about a bare GUID and about the same
  GUID with `.1c`, `.9` and `.7`
- **THEN** it answers yes to each

#### Scenario: A name no statement matches
- **WHEN** the predicate is asked about `DBNames`, about a GUID with
  another suffix, or about a name that is not a GUID
- **THEN** it answers no

## MODIFIED Requirements

### Requirement: Read the resources of a configuration extension
The library SHALL read the content-addressed store of the configuration
extensions: `_ExtensionsInfo` names every extension and carries, in
`ExtensionZippedInfo` after a four-byte marker, the twenty-byte key of
its root resource; the store `ConfigCAS` holds every resource under the
lower-case hexadecimal of its key; and the root resource lists the
resources of the extension as pairs of a name and the base64 of the key
of its content. The library SHALL provide, for both providers, a
statement probing whether the base carries `_ExtensionsInfo`, a statement
reading the extensions, and a statement reading the parts of one stored
resource by its key.

The statement reading the extensions SHALL answer, for each extension and
in ascending `_ExtensionOrder`: the reference `_IDRRef` that identifies
it, that order, the name `_ExtName`, and the `_ExtensionZippedInfo`
record.

A resource that carries several records one after another — the root
does — SHALL be parsed into the sequence of its records.

`_ExtensionZippedInfo` SHALL decode into a typed record rather than being
treated as opaque: after the marker and the root key it is a
tag-length-value stream carrying the synonym of the extension as a
UTF-16 string and its version as an ASCII string.

Whether the base applies the extension SHALL be answered only from the
shape it was measured in: a record ending in the terminator, with the
flag three bytes from its end, `0x82` where the base applies the
extension and `0x81` where it does not. A record of any other shape, or
carrying any other value there, SHALL answer that the activity is
**unknown**. Decoding SHALL NOT derive the flag by scanning for
high-bit bytes: a length-bearing tag the decoder does not know carries
payload bytes indistinguishable from flags, and reading one of those as
the flag would report an extension the base applies as inactive.

Decoding SHALL answer `None` only when the record is too short to carry
the marker and the root key. A stream that ends inside a field, or
carries a tag the decoder does not know, SHALL answer the root key and
whatever was decoded before it, with the activity unknown.

#### Scenario: The index of an extension
- **WHEN** the root resource of `_ДемоРасширение` is parsed
- **THEN** it answers 169 resources, among them
  `262144f7-02b6-4906-89fd-297cc72fe383.0` with the key of the rights of
  that role

#### Scenario: A blob that is no extension record
- **WHEN** `ExtensionZippedInfo` is shorter than its marker and key
- **THEN** reading the root key answers none

#### Scenario: The record of an extension
- **WHEN** the `_ExtensionZippedInfo` of `_ДемоРасширение` is decoded
- **THEN** the record answers its root key, its version and its synonym

#### Scenario: A tag the decoder does not know
- **WHEN** the stream carries an unknown length-bearing tag whose payload
  bytes have the high bit set
- **THEN** the activity is unknown, and no payload byte is read as the
  flag

#### Scenario: A field that ends early
- **WHEN** the stream ends inside a string the decoder does know
- **THEN** the root key and what was decoded before it are answered, and
  the activity is unknown

#### Scenario: An extension the base does not apply
- **WHEN** the records of two extensions of one base, alike but for the
  platform applying one and not the other, are decoded
- **THEN** they differ in the root key, in the synonym naming them apart
  and in that one flag, and only the second is read as inactive

#### Scenario: A record carrying nothing after the key
- **WHEN** the record stops right after the root key
- **THEN** the key is answered, the synonym and the version are absent,
  and the activity is unknown
