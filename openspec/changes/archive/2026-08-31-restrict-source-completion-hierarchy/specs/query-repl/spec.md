## ADDED Requirements

### Requirement: Complete query sources through the metadata hierarchy
The interactive console SHALL use a dedicated source-completion catalog after
`ИЗ`/`FROM` and `СОЕДИНЕНИЕ`/`JOIN`. Every ordinary source candidate SHALL
have the form `Type.MetadataName`, where `Type` is the resolved object's
Russian or English metadata kind. Source completion SHALL exclude bare object
names, field aliases, reference paths, and physical PostgreSQL table names.

#### Scenario: Empty source prefix
- **WHEN** the user presses Tab immediately after `ИЗ` or `FROM`
- **THEN** every metadata candidate is qualified as `Type.MetadataName`

#### Scenario: Partial metadata type
- **WHEN** the user enters a partial type after a source keyword
- **THEN** completion offers matching qualified objects of that type and does
  not offer fields or physical tables

#### Scenario: Register virtual source
- **WHEN** a qualified register source is followed by a partial virtual-table
  suffix
- **THEN** completion offers only virtual tables valid for that register kind

#### Scenario: Non-source field completion
- **WHEN** completion occurs outside a source position
- **THEN** field aliases and qualified reference paths remain available
