## MODIFIED Requirements

### Requirement: Discover resource names and headers without logical detail decoding
Both session forms SHALL provide a bounded deterministic list of physical Config resource names without reading BinaryData. A bounded header decoder SHALL expose measured declaration identities and localized presentation facts without constructing detailed descriptors or attributes. It SHALL reuse established header identity rules and reject malformed data.

#### Scenario: Missing module slot
- **WHEN** a configuration object has no manager or object-module slot in the filename index
- **THEN** discovery does not require an attempted body read to learn that it is absent

#### Scenario: Header discovery
- **WHEN** a supported descriptor contains nested attributes
- **THEN** header discovery returns the same own identity and name as full decoding without materializing the attributes

## ADDED Requirements

### Requirement: Discover measured generated types and section identities
Bounded Config header discovery SHALL expose supported measured reference/object generated type identities and tabular-section/enum-value GUID/name identities with existing localized synonyms and measured business-process route-point reference types and recognized member GUIDs without constructing detailed attribute descriptors. Unknown forms SHALL remain distinct from measured absence.

#### Scenario: Native header parity
- **WHEN** supported demo, buh or unf descriptors are projected lightly
- **THEN** generated identities and section membership agree with the detailed measured decoder

#### Scenario: Immutable declaration presentations
- **WHEN** a consumer registers immutable enum members and business-process type aliases
- **THEN** lightweight discovery preserves measured own/member synonyms and the measured route-point reference type without decoding attribute details

#### Scenario: Resolve a dependency naming a nested member
- **WHEN** a functional option names an attribute GUID rather than its owner's GUID
- **THEN** header discovery provides the recognized member-to-resource ownership facts with the same identity rule as detailed decoding

### Requirement: Discover direct query metadata references
The SDK SHALL project direct metadata kind/name references from its existing bounded query parser for sources, nested queries, VALUE, TYPE, reference casts and reference tests. It SHALL use existing case folding to deduplicate and SHALL exclude aliases, literals and temporary tables. Constants SHALL retain statically distinguishable referenced names; a star or unresolved ambiguity MAY request all constants. The result SHALL NOT claim a transitive or RLS closure.

#### Scenario: Nested metadata expressions
- **WHEN** a query refers to objects through nested sources and metadata expressions
- **THEN** discovery returns structured deduplicated references without requiring a resolved snapshot

#### Scenario: Constants selection
- **WHEN** constant fields are distinguishable from aliases and other sources
- **THEN** discovery returns only their names; ambiguous references or an actual star explicitly request the whole constants collection

#### Scenario: Malformed input
- **WHEN** input is malformed or exceeds existing parser limits
- **THEN** discovery returns the ordinary positional query diagnostic

### Requirement: Acquire extension physical metadata independently
Async and blocking sessions SHALL expose bounded acquisition of existing extension restructure metadata with cached layout and read-only cleanup, without Config or ConfigCAS acquisition.

#### Scenario: Reuse discovery
- **WHEN** physical extension metadata is acquired independently
- **THEN** only the existing extension restructure reader/decoder is used and the returned structures can be supplied to explicit-resource resolution
