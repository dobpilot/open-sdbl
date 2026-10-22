## ADDED Requirements

### Requirement: Finalize queries from resolved runtime facts
The core SHALL expose explicit query finalization from immutable RuntimeMetadataFacts without repeating physical selection, extension merging or descriptor normalization. Complete fields, query indices, comparison reports and fingerprints SHALL match ordinary complete resolution for the same inputs. Finalization SHALL preserve the original runtime facts.

#### Scenario: A query follows runtime hydration
- **WHEN** a caller finalizes already resolved runtime facts
- **THEN** the resulting complete snapshot matches ordinary complete resolution and the borrowed runtime facts remain unchanged

#### Scenario: Accumulated resources grow
- **WHEN** new resources are admitted after older runtime facts were published
- **THEN** finalizing older facts preserves their old composition and finalizing new facts reflects the new composition

### Requirement: Strict cache finalization
ConfigurationMetadataCache SHALL expose finalization of caller-selected runtime facts with the same missing and unsupported separator descriptor diagnostics as ordinary cached resolution. It SHALL perform no decoding, SQL or I/O.

#### Scenario: Required bound parameter is missing
- **WHEN** supplied runtime facts lack a required separator parameter descriptor
- **THEN** strict cache finalization returns the same descriptor diagnostic as ordinary cached resolution
