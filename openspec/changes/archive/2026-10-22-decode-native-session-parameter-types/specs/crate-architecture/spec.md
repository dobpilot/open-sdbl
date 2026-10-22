## ADDED Requirements

### Requirement: Decode measured native session parameter declarations
For the measured native root typed session parameter envelope, metadata acquisition SHALL preserve the complete declared type Pattern in the owning ConfigDescriptor. Raw reference identifiers and primitive qualifiers SHALL remain unchanged. Unknown or incomplete declarations and unrecognized envelopes SHALL remain unknown rather than returning a successful prefix or borrowing another descriptor's Pattern.

#### Scenario: Native UUID session parameter
- **WHEN** the paired native DEMO UUID session parameter resource is decoded
- **THEN** its descriptor contains the raw UUID reference declaration matching the XML and EDT type

#### Scenario: Incomplete or foreign typed envelope
- **WHEN** a Pattern member is unknown or the measured envelope or owner identity does not match
- **THEN** no successful declared type list is published for that envelope
