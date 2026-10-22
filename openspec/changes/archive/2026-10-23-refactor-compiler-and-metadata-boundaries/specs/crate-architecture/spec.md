## ADDED Requirements

### Requirement: Share compiler semantic policies across source contexts
Source-free and sourced compilation SHALL use shared internal policies for
scalar comparison eligibility and exact logical projection availability. Name
resolution, authorization and dialect rendering SHALL remain explicit and
preserve existing public behavior.

#### Scenario: Equivalent resolved operands
- **WHEN** equivalent resolved scalar kinds appear with and without FROM
- **THEN** the same comparison and projection policies apply, including documented contextual exceptions

### Requirement: Keep native role decoding in the pure core
Recognition and validation of native empty-role resources SHALL reside in the
core metadata decoder. The database crate SHALL orchestrate acquisition without
interpreting native role serialization or manufacturing compressed decoder input.
The core SHALL expose `parse_role_without_rights(&Guid, &[u8]) -> Option<RoleRights>`
using the default decoded-byte ceiling. It SHALL return empty rights only for
the measured complete own-role descriptor matching the supplied non-nil GUID;
unknown, malformed, borrowed, mismatched or rights-bearing descriptors SHALL
return None. Existing rights-resource decoder signatures and behavior SHALL
remain unchanged.

#### Scenario: Acquired empty-role resource
- **WHEN** acquisition supplies a measured valid empty-role descriptor
- **THEN** core decoding produces typed empty rights while malformed or unknown formats remain distinguishable

#### Scenario: Descriptor does not prove empty rights
- **WHEN** the descriptor is malformed, has an unsupported shape, promises a rights subobject or names another role
- **THEN** the core returns None and acquisition does not treat the role as successfully loaded

### Requirement: Validate runtime facts through focused internal records
Runtime metadata projection SHALL separate format recognition, validation and
application through focused internal modules and validated records. Existing
public descriptor types, supported formats, fact ordering, diagnostics and
partial-result behavior SHALL remain compatible.

#### Scenario: Malformed family record
- **WHEN** a native record is malformed within a supported fact family
- **THEN** its existing diagnostic and partial-result behavior are preserved without publishing unvalidated facts
