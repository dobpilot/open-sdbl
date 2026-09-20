## ADDED Requirements

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

A resource that carries several records one after another — the root
does — SHALL be parsed into the sequence of its records.

#### Scenario: The index of an extension
- **WHEN** the root resource of `_ДемоРасширение` is parsed
- **THEN** it answers 169 resources, among them
  `262144f7-02b6-4906-89fd-297cc72fe383.0` with the key of the rights of
  that role

#### Scenario: A blob that is no extension record
- **WHEN** `ExtensionZippedInfo` is shorter than its marker and key
- **THEN** reading the root key answers none
