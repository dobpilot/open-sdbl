## MODIFIED Requirements

### Requirement: Describe each acquired extension
Each extension a whole-configuration read answers SHALL carry, beside its
identity, name, activity, order and resources: its version, the content
hash of its root, whether safe mode is on, whether it is used in a
distributed information base, its scope, the catalog of the classes and
objects its configuration declares, and the object headers of its
descriptors with the explicit link an adopted object carries to the
main-configuration object it adopts.

A value the reader does not decode reliably SHALL be an `Option` and SHALL
be `None` rather than a default: the version when the record carries none,
safe mode for unsupported records, the distributed-base flag and the scope
when the base lacks the column, the catalog when the configuration root is
missing or not in the measured shape, and an object's link when it is
nil. The link SHALL be read from the adopted object's own header; the name
a resource has in `ConfigCAS` SHALL NOT be taken as a link. Both providers
SHALL answer the same row type and share every step after it.

The object headers SHALL be answered whole or not at all, as an `Option`
of the list. The list SHALL be `Some` only when every description
resource of the extension — every resource its root index names by a bare
identifier — inflated within the per-resource limit and parsed, the
catalog is known, and every object the catalog declares has a header of
its own read from the resource named by that object. In every other case
the list SHALL be `None`; a shorter list SHALL NOT be answered as the
list. Resources whose name carries a suffix are not descriptions and SHALL
NOT affect completeness. Completeness SHALL add no data: a header whose
tail is not in the measured shape keeps its belonging and link unknown,
and an adopted object recording the nil identifier keeps no link.

#### Scenario: An applied and a not-applied extension
- **WHEN** a base carries one extension it applies and one it does not
- **THEN** both are answered with identity, order, activity `Some(true)`
  and `Some(false)`, and their own content hash

#### Scenario: An adopted catalog
- **WHEN** an extension adopts a catalog of the main configuration by
  identifier
- **THEN** its object list carries that catalog as adopted, with the
  identifier of the main-configuration catalog as its link

#### Scenario: An object of the extension's own
- **WHEN** an extension declares a catalog of its own
- **THEN** that object is answered as own, without a link

#### Scenario: A configuration root that is not understood
- **WHEN** the configuration root resource of an extension does not have
  the measured shape
- **THEN** the extension is answered with no catalog rather than a partial
  one and no object headers, since there is nothing to check them
  against, and everything else as before

#### Scenario: Safe mode
- **WHEN** an extension is read in the measured complete runtime-property record shape
- **THEN** safe mode is the decoded Boolean

#### Scenario: Unsupported safe mode record
- **WHEN** the extension's runtime-property record is truncated or unsupported
- **THEN** safe mode is unknown

#### Scenario: A base without the new columns
- **WHEN** `_ExtensionsInfo` has neither `_ExtensionScope` nor
  `_UsedInDistributedInfoBase`
- **THEN** the extensions are read, with scope and distributed-base use
  unknown

#### Scenario: Every description decoded
- **WHEN** every description resource of an extension decodes and every
  declared object has its header
- **THEN** the object headers are answered, all of them

#### Scenario: A damaged description
- **WHEN** one description resource of an extension does not inflate or
  does not parse
- **THEN** the object headers are unknown, rather than answered without
  that resource's headers

#### Scenario: A declared object without a header
- **WHEN** the catalog declares an object whose own resource carries no
  header for it
- **THEN** the object headers are unknown

#### Scenario: A damaged resource that is no description
- **WHEN** a resource whose name carries a suffix, such as a module, does
  not decode
- **THEN** the object headers are answered as if it were intact

## ADDED Requirements

### Requirement: Answer measured native extension runtime properties
Acquired extensions SHALL carry the native platform hash sum separately from the existing hexadecimal content hash, the decoded safe-mode Boolean, the security profile name and a typed known scope alongside the raw scope code. Known empty profile names SHALL remain distinguishable from unknown profiles. Unknown, truncated or structurally unsupported mode/profile records SHALL return unknown values rather than guessed defaults. Counted profile strings SHALL NOT be treated as extension versions.

#### Scenario: Safe mode is explicitly disabled
- **WHEN** a complete measured record encodes safe mode false and no profile
- **THEN** its mode is false and its known profile name is empty

#### Scenario: A named security profile
- **WHEN** a complete measured record carries a security profile name
- **THEN** the profile string is preserved separately from the mode Boolean and version

#### Scenario: The native generation hash
- **WHEN** a record is in the measured native generation-key shape
- **THEN** the platform hash sum uses the measured Base64 generation key representation, independently of the hex CAS content hash

#### Scenario: The measured InfoBase scope
- **WHEN** the raw scope code is 1
- **THEN** its known scope is InfoBase

#### Scenario: An unmeasured scope
- **WHEN** the raw scope code has not been mapped on the native platform
- **THEN** its raw value remains available and its typed scope is unknown

#### Scenario: A malformed property record
- **WHEN** a mode/profile field or the record's structural walk is incomplete or unsupported
- **THEN** mode and profile are unknown
