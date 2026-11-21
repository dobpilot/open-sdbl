## ADDED Requirements

### Requirement: Authorize measured information-register programmatic fields
For a recognized independent information-register owner with complete dimensions and resources, known write mode and periodicity, the SDK SHALL map verified declared dimension/resource fields to object Read. It SHALL map Period only for the measured Day periodicity. UI View/Edit and attribute-default flags SHALL NOT replace owner Read. Foreign fields, unknown declarations, incomplete owners and unmeasured standard fields SHALL fail closed. Existing complete-role, explicit member Read, role-union, selected-principal and RLS boundaries SHALL remain effective.

#### Scenario: Owner Read without attribute defaults
- **WHEN** a selected user's loaded role grants register Read with either value of attribute defaults
- **THEN** verified dimension/resource reads and a Day-periodic register's Period pass

#### Scenario: Defaults do not grant owner Read
- **WHEN** no selected role grants register Read, even with attribute defaults enabled
- **THEN** all these field reads fail

#### Scenario: Unknown register field shape
- **WHEN** the field belongs elsewhere, collections or declarations are incomplete, or Period is requested outside the measured Day periodicity
- **THEN** the field check refuses rather than authorizing by owner Read alone

### Requirement: Authorize measured document programmatic fields
For a recognized document owner with complete own attribute membership and recognized document settings, the SDK SHALL map its reference and own attributes with verified nonempty declared types to object Read independently of UI View/Edit and attribute defaults. It SHALL retain explicit member Read overrides and complete-role/role-union behavior. Unknown standard fields, foreign attributes, unsupported tabular sections and unverified composition SHALL fail closed. Ordinary query and RLS enforcement SHALL NOT be bypassed.

#### Scenario: Document reference and own field are readable
- **WHEN** the selected role grants document Read with defaults disabled or enabled
- **THEN** its verified reference and own declared field pass programmatic field checks

#### Scenario: Document owner Read is absent
- **WHEN** the selected user's roles do not grant document Read
- **THEN** defaults and UI rights do not authorize reference or own-field reads

#### Scenario: Unsupported document membership
- **WHEN** the requested attribute is foreign, the owner is incomplete or the path is a tabular section
- **THEN** the field check refuses explicitly before SQL
