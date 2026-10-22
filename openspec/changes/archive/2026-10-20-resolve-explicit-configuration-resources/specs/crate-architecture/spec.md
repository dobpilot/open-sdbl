## ADDED Requirements

### Requirement: Acquire reusable physical metadata inputs
Asynchronous and blocking sessions SHALL provide DBNames, SchemaStorage and live catalog discovery with the detected layout without reading Config, ConfigCAS or constructing logical object details. The returned inputs SHALL be reusable by pure resolution without further SQL.

#### Scenario: Repeated hydration
- **WHEN** a caller acquires physical inputs once and resolves two explicitly supplied resource sets
- **THEN** DBNames, SchemaStorage and live catalog are read once and neither resolution performs database I/O

### Requirement: Resolve explicitly supplied resources only
The SDK SHALL build a MetadataSnapshot and ResolutionReport from explicitly supplied Config resources, explicit extension structures and reusable physical inputs. It SHALL reuse existing resource decoders and resolver, project physical inputs to the reached objects and their tables/fields, and SHALL NOT fetch additional Config or ConfigCAS resources. Missing related settings SHALL remain unknown or diagnosed, never guessed defaults. Separator flags SHALL survive projection.

#### Scenario: One reached object
- **WHEN** explicit resources describe one object in a larger physical index
- **THEN** its supported query mappings agree with whole resolution and unrelated logical objects are not materialized

#### Scenario: Missing separator descriptor
- **WHEN** a selected table contains a known separator whose settings resource was not supplied
- **THEN** resolution fails with machine-readable missing descriptor GUIDs before returning a usable snapshot rather than silently disabling separation

#### Scenario: Explicit extension structure
- **WHEN** callers supply extension DBNames, schema and descriptors
- **THEN** the selected metadata preserves their supported physical mappings without acquiring extension bodies

#### Scenario: Missing bound parameter descriptor
- **WHEN** supplied separator settings reference an unacquired parameter descriptor
- **THEN** the helper reports that parameter GUID as a missing dependency rather than substituting a fallback parameter name

### Requirement: Discover resource names and headers without logical detail decoding
Both session forms SHALL provide a bounded deterministic list of physical Config resource names without reading BinaryData. A bounded own-header decoder SHALL return only the resource's measured own GUID and name without constructing detailed descriptors or attributes. It SHALL reuse established header identity rules and reject malformed data.

#### Scenario: Missing module slot
- **WHEN** a configuration object has no manager or object-module slot in the filename index
- **THEN** discovery does not require an attempted body read to learn that it is absent

#### Scenario: Header discovery
- **WHEN** a supported descriptor contains nested attributes
- **THEN** header discovery returns the same own identity and name as full decoding without materializing the attributes

### Requirement: Preserve identities and access caches during hydration
Objects and fields SHALL retain their GUID-based identities across growing or reordered metadata subsets. A role-only AccessStore SHALL retain loaded users and rights when used with later query snapshots without reset. Prepared query fingerprint checks SHALL remain enforced across snapshot changes.

#### Scenario: Grow a query subset
- **WHEN** one access store is used with two query snapshots whose object order and extent differ
- **THEN** the same GUID has the same object/field identity and the same loaded role right is evaluated without losing cached users or rights
