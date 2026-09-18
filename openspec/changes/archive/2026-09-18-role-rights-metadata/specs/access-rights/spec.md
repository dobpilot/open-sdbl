## ADDED Requirements

### Requirement: Decode the rights resource of a role
The library SHALL decode a role's `<guid>.0` Config resource into
`RoleRights`: the header flags `setForNewObjects`,
`setForAttributesByDefault` and `independentRightsOfChildObjects`; one
`ObjectRights` per metadata object or object member the resource lists,
each right with whether it is granted (`1`) or refused (`-1`) and its
restriction conditions with their field identifiers; and the restriction
templates with their signature and body. The resource SHALL be decoded
by shape, whatever its file name, so a `ConfigCas` resource decodes the
same way. A resource of another format version SHALL be refused with a
`MetadataError`.

#### Scenario: A read-only role with a restriction
- **WHEN** the rights resource of `ЧтениеЭлектронныхДокументов` (БП 3.0)
  is decoded
- **THEN** the first object grants `Read`, `View` and `InputByString`,
  `Read` carries the condition starting with
  `#Если &ОграничениеДоступаНаУровнеЗаписейУниверсально #Тогда`, and the
  four templates `ДляОбъекта`, `ДляРегистра`, `ПоЗначениям`,
  `ПоЗначениямРасширенный` are listed

#### Scenario: Refused rights
- **WHEN** a rights list records `-1` for a right
- **THEN** that right is present with `allowed == false`

### Requirement: Name the standard rights
`Right` SHALL name the standard right identifiers by their platform
names — `Read`, `Insert`, `Update`, `Delete`, `View`, `Edit`,
`InputByString`, the interactive and data-history rights, the document
posting rights, `TotalsControl`, `Use`, the business-process and task
rights, the session-parameter rights `Get` and `Set`, and the
configuration rights from `Administration` to `Output` — with their
Russian synonyms where the platform has one, and SHALL carry any other
identifier as `Right::Other(guid)`.

#### Scenario: The reading right
- **WHEN** the identifier `1c87578f-9e09-4ec0-a991-5629c87b1588` is named
- **THEN** it is `Right::Read`, spelled `Чтение`

#### Scenario: An unknown identifier
- **WHEN** an identifier the table does not list is named
- **THEN** it is `Right::Other` with that identifier

### Requirement: Catalog the roles of a configuration
The library SHALL list the roles a configuration declares from the roles
collection of the configuration root and name them from their bare-GUID
descriptors: `RoleCatalog` SHALL answer every role with its identifier,
name and synonym, and SHALL find a role by name or by identifier.

#### Scenario: Roles named from descriptors
- **WHEN** the root resource lists two role identifiers and the
  descriptors of both are parsed
- **THEN** the catalog names both and finds `АдминистраторСистемы` by
  name and by identifier
