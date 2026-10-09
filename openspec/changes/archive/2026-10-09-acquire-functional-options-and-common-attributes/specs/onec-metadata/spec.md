## ADDED Requirements

### Requirement: Acquire functional-option metadata
The SDK SHALL retain typed measured constant, catalog-attribute and information-register-resource location GUIDs, explicit privileged retrieval flags and complete ordered functional-option parameter Use links. Unknown, malformed or incomplete lists SHALL remain unknown without partial success. Known absence SHALL remain distinct from unknown forms. SDK SHALL NOT evaluate BSL.

#### Scenario: Native sources and parameters
- **WHEN** paired native XML and CF declare supported option sources, flags and parameter links
- **THEN** acquired descriptors retain those identities and values after source resource removal

#### Scenario: Unsupported parameter links
- **WHEN** any parameter link or list is invalid or incomplete
- **THEN** the whole list remains unknown

### Requirement: Acquire common-attribute runtime settings
The SDK SHALL distinguish measured DontUse and Separate modes independently of separation settings presence, preserve explicit Config usage settings and independently known conditional absence, and explicitly distinguish these settings from full platform content. Unsupported applicability, ordering and conditional forms SHALL remain unknown.

#### Scenario: Independent separation mode
- **WHEN** native controls declare different separation modes with settings present
- **THEN** acquired mode matches native metadata without inference from physical fields

#### Scenario: Explicit settings versus full content
- **WHEN** Config contains only explicit settings
- **THEN** the SDK does not publish that list as complete platform content or guess missing Auto entries

### Requirement: Acquire functional-option source constraints
The SDK SHALL retain measured catalog attribute ForItem, ForFolder and ForFolderAndItem usage and independent or recorder-subordinate information-register write mode, independently from measured nonperiodical or day periodicity. Unsupported codes SHALL remain unknown. These descriptors SHALL NOT implement option evaluation.

#### Scenario: Native source profiles
- **WHEN** independently dumped native XML and CF contain the supported source profiles
- **THEN** acquired settings preserve the native property values and source identity

#### Scenario: Unknown source profile
- **WHEN** a layout or setting code is unsupported
- **THEN** that property remains unknown without inference from physical tables

### Requirement: Acquire complete measured common-attribute composition
The SDK SHALL publish complete runtime content separately from explicit Config settings only when the full configuration inventory, separation mode and all applicable owner identities are acquired. For the measured native configuration profile it SHALL preserve ascending GUID iteration order, Auto for implicit content, explicit Auto/Use/DontUse, independently known Undefined and Boolean-constant conditional separation identity. In Separate mode constants SHALL participate and document journals and sequences SHALL not participate; in DontUse mode the inverse SHALL hold. Unknown inventory, missing applicable owners or unsupported nonempty external-source collections SHALL make the whole content unknown.

#### Scenario: Native applicability and iteration
- **WHEN** paired native CF/XML and runtime receipts enumerate supported configuration classes in each separation mode
- **THEN** complete acquired composition matches native identities, usage and iteration order, including implicit entries

#### Scenario: Explicit settings reordered
- **WHEN** two native controls reverse explicit Config Items
- **THEN** complete content retains the independently measured GUID order and conditional constant identity

#### Scenario: Incomplete acquired composition
- **WHEN** a required owner is missing or configuration inventory cannot be verified
- **THEN** no partial list is published as complete content

#### Scenario: Scheduled-job applicability
- **WHEN** native Separate and DontUse controls contain a disabled scheduled job
- **THEN** complete content includes its metadata identity only in Separate mode, without implementing or executing scheduled jobs

### Requirement: Acquire complete information-register source field membership
The SDK SHALL acquire complete ordered dimension and resource GUID collections for the measured information-register source profile. Unknown, malformed, duplicate, incomplete or unresolved members SHALL make that entire collection unknown rather than a partial list. Verified empty SHALL remain distinct from unknown.

#### Scenario: Native information-register collections
- **WHEN** paired native XML/CF contain counted dimension and resource collections
- **THEN** acquired source descriptors retain the complete membership independently of individual field-purpose labels and source resource retention

#### Scenario: Missing dimension or resource
- **WHEN** any member count, identity or acquired descriptor is missing
- **THEN** the entire affected collection remains unknown
