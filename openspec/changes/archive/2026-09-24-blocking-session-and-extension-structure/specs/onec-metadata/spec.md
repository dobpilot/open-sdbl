## ADDED Requirements

### Requirement: Read what the extensions table records about an extension
The library SHALL provide, for both providers, a statement probing whether
`_ExtensionsInfo` carries `_ExtensionScope` and
`_UsedInDistributedInfoBase`, and a statement reading the extensions that
answers, for each extension and in ascending `_ExtensionOrder`: the
reference `_IDRRef`, that order, the name `_ExtName`, the
`_ExtensionZippedInfo` record, the `_ExtensionScope` code as an integer,
and `_UsedInDistributedInfoBase` as the integer `0` or `1`. For a column
the probe did not find, the statement SHALL answer `NULL` in its place and
SHALL NOT name the column. The statement SHALL be built from fixed
fragments only.

#### Scenario: A current base
- **WHEN** both columns exist
- **THEN** the statement reads them and answers them as integers

#### Scenario: An older base
- **WHEN** neither column exists
- **THEN** the statement names neither and answers `NULL` for both

### Requirement: Decode the catalog of an extension configuration
The library SHALL answer the resource holding an extension's configuration
object from the extension's root index, where a record `{2,<guid>,…}`
names it, and SHALL decode that resource into the configuration's own
identifier and, in stored order, each class with the identifiers of the
objects the extension declares in it. The walk SHALL follow the measured
shape of the configuration root — a count of groups, each group a class
and its optionally versioned inclusion list, each inclusion list a count
of entries, each entry a class, a count and that many identifiers — and
SHALL answer an error for any entry of another shape rather than skip it.
Classes SHALL be answered by identifier; the library SHALL NOT name them.

#### Scenario: The catalog of `Расширение1`
- **WHEN** the configuration root of `Расширение1` of the demo base is
  decoded
- **THEN** catalogs answer `СтавкиНДС` and `Расш1_Справочник1`, roles
  answer `Расш1_ОсновнаяРоль`, and languages answer `Русский`

#### Scenario: A root index without a configuration record
- **WHEN** a root index carries only name/key pairs
- **THEN** no configuration root is answered

#### Scenario: A malformed entry
- **WHEN** an entry's count does not match the identifiers that follow it
- **THEN** decoding answers an error

### Requirement: Decode object headers and their links
The library SHALL answer every object header of a description resource —
the object's identifier, name, whether it belongs to the extension or is
adopted, and the identifier of the main-configuration object it extends.
A header is `{3,{1,0,<id>},<name>,<synonym>,<comment>,<belonging>,<n>,
(<guid>,<tag>)×n,<extends>,<x>}`. Belonging SHALL be answered only for the
measured values `0` (own) and `1` (adopted) and a tail of exactly that
length; anything else SHALL answer it unknown. The link SHALL be answered
only for an adopted object whose `<extends>` is a non-nil identifier; the
pairs SHALL NOT be interpreted.

#### Scenario: An adopted catalog and its attribute
- **WHEN** the description of `СтавкиНДС` in `Расширение1` is decoded
- **THEN** the catalog is adopted and extends
  `6a386de9-b78e-47cb-a7e8-864b52022b24`, its adopted attribute `Ставка`
  extends `9a871782-6f75-4078-a8f2-a33d9dcb8c7f`, and its own attribute
  `Расш1_Реквизит1` is own with no link

#### Scenario: Adopted by name
- **WHEN** an adopted object carries the nil identifier as `<extends>`
- **THEN** it is answered as adopted with no link

#### Scenario: An unfamiliar tail
- **WHEN** a header's tail is shorter than its pair count says
- **THEN** belonging and link are unknown, and the identifier and name are
  still answered
