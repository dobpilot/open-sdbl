## ADDED Requirements

### Requirement: Recognize a complete extension role with no rights subobject
When an extension index declares a role descriptor but no rights resource for that role, the database fallback SHALL recognize an empty complete rights set only from the complete measured native Role descriptor form explicitly declaring no rights subobject, with the role's matching own identifier. The empty set SHALL contain no object rights, no restriction templates and no default grants. Unknown forms and unavailable descriptors SHALL remain unreadable. A declared rights resource SHALL continue to use the existing decoder; unavailable or malformed content SHALL NOT be replaced with empty rights.

#### Scenario: Measured role without a rights subobject
- **WHEN** the extension carries the preserved native descriptor of `ДемоПроф_ОбщиеПрава` and its index declares no `<guid>.0`
- **THEN** its loaded rights are complete and empty without granting an additional permission

#### Scenario: Descriptor promises rights or has an unknown form
- **WHEN** the rights resource is absent but the descriptor declares a rights subobject, is incomplete, belongs to another identity or uses an unmeasured form
- **THEN** the role's rights remain unreadable

#### Scenario: Declared rights resource is missing or corrupt
- **WHEN** the index declares `<guid>.0` but its content is unavailable or malformed
- **THEN** the fallback retains its existing failure or unread result and does not substitute empty rights
