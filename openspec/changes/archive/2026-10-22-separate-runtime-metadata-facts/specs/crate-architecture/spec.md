## ADDED Requirements

### Requirement: Runtime metadata facts without query snapshot finalization
The core SHALL expose RuntimeMetadataFacts as a distinct immutable runtime-only view containing normalized descriptors, resolved objects and values, schema, separator facts, criteria and roles. It SHALL expose common_attribute_separation(Guid) distinguishing missing or ambiguous physical fields from ordinary fields and separator fields without constructing ordinary query fields. It SHALL NOT expose a query fingerprint or permit query preparation as a MetadataSnapshot. MetadataSnapshot SHALL expose runtime_facts() borrowing the same common facts without cloning.

#### Scenario: Runtime facts agree with full resolution
- **WHEN** the same selected inputs, admitted resources and extensions are resolved as runtime facts and as a complete snapshot
- **THEN** every exposed runtime fact and object identity lookup agrees with the complete snapshot
- **AND** runtime-only resolution does not build ordinary query fields, query indexes, comparison reports or a fingerprint

### Requirement: Strict cached runtime metadata resolution
ConfigurationMetadataCache SHALL expose resolve_runtime using already admitted parsed resources without parsing, decoding, SQL or I/O. It SHALL apply the same extension merging, normalization and strict separator descriptor validation as complete cached resolution. Existing complete resolution, fingerprints, reports and prepared-query snapshot ownership SHALL remain unchanged.

#### Scenario: Growing runtime resources
- **WHEN** immutable resources are admitted incrementally and runtime facts are requested after each growth
- **THEN** facts agree with complete resolution for that accumulated resource set and previously returned results remain unchanged

#### Scenario: Missing separator dependencies
- **WHEN** runtime resolution lacks a required separator or bound parameter descriptor
- **THEN** it returns the same typed missing dependency or unsupported settings error as complete resolution

#### Scenario: Common attribute classification preserves uncertainty
- **WHEN** selected physical facts contain zero, one ordinary, one separator or multiple fields for a common attribute GUID
- **THEN** classification returns None, Some(false), Some(true) or None respectively, matching complete snapshot field lookup
