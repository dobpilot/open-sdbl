## ADDED Requirements

### Requirement: Distinguish a known runtime version from an unknown record
Acquired extensions SHALL provide an authoritative runtime version separately from legacy partial decoding. A complete measured record with an absent version SHALL return a known empty string. Unsupported or incomplete records SHALL return an unknown value.

#### Scenario: No configured version
- **WHEN** a complete measured record contains the native absent-version marker
- **THEN** its runtime version is known and empty

#### Scenario: Unsupported record
- **WHEN** the runtime record is incomplete or unsupported
- **THEN** its runtime version is unknown even if legacy decoding found a printable field

#### Scenario: Configured version
- **WHEN** a complete measured record carries a version string
- **THEN** that string is preserved as its runtime version
