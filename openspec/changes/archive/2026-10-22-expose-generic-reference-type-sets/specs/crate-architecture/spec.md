## ADDED Requirements

### Requirement: Classify native generic reference declarations
The core SHALL expose ReferenceTypeSet with Kind(MetadataKind) and Any, ConfigDeclaredType::reference_type_set(), and ReferenceTypeSet::contains_kind(). Classification SHALL reuse the existing measured generic GUID mapping only for reference-tag declarations. It SHALL preserve raw declarations and qualifiers and SHALL NOT interpret an unrecognized reference as an empty successful type list.

#### Scenario: Native generic families
- **WHEN** the paired native DEMO generic attribute is decoded
- **THEN** its eight known families are classified in source order without rewriting its raw GUIDs

#### Scenario: Unknown or primitive declaration
- **WHEN** a primitive declaration or unrecognized reference GUID is classified
- **THEN** no family is reported and its original declaration remains unchanged

### Requirement: Preserve generic families through normalization
Known generic families SHALL remain classifiable after existing named alias resolution and in runtime facts, complete snapshots and explicit finalization. Classification SHALL NOT expand only the reached subset as if it were a complete family, change SQL/rights semantics, or return successful partial lists for previously unknown normalization states. Membership SHALL use the same reference kinds as existing SQL category handling.

#### Scenario: Named alias includes a family and a qualified primitive
- **WHEN** the alias is resolved
- **THEN** the generic family remains recognizable and primitive qualifiers are preserved in all metadata views

#### Scenario: A lazy snapshot has only one reached catalog
- **WHEN** its attribute declares every catalog reference
- **THEN** the family remains explicit rather than being replaced by that single catalog
