## ADDED Requirements

### Requirement: Decode one bounded V8 container entry
The dependency-free SDK SHALL expose pure decoding of one named V8 container entry. It SHALL return the exact assembled entry bytes or known absence. Malformed headers, invalid UTF-16 names, truncated or cyclic block chains and documents exceeding the caller-provided per-document bound SHALL return metadata errors. The bound SHALL apply independently to directory, description and payload documents; it SHALL NOT promise an aggregate memory ceiling.

#### Scenario: Native scalar setting
- **WHEN** the own native scalar settings container is read at $Stream$
- **THEN** the exact measured internal scalar stream is returned

#### Scenario: Native multipart value storage
- **WHEN** the own native storage container is read at its root and auxiliary entry names
- **THEN** both exact measured streams are independently available

#### Scenario: Missing or invalid entry
- **WHEN** the entry is absent in a valid container
- **THEN** the result is known absence
- **WHEN** an assembled document is malformed or exceeds its bound
- **THEN** decoding fails rather than returning absence or partial bytes

### Requirement: Decode measured common-setting text
The SDK SHALL distinguish ordinary internal value text from the decoded contents of a multipart ValueStorage setting. It SHALL resolve the measured ValueStorage auxiliary reference only inside its container and reuse the existing bounded stored-value codec. Missing or malformed required streams and unsupported ValueStorage reference forms SHALL fail explicitly. The SDK SHALL NOT construct BSL runtime values.

#### Scenario: Scalar setting
- **WHEN** the native own scalar setting is decoded
- **THEN** the result is ordinary internal text representing number 17

#### Scenario: Multipart storage setting
- **WHEN** the native own ValueStorage setting is decoded
- **THEN** the result is identified as ValueStorage contents and retains the measured internal map text
- **AND** an absent auxiliary document is an error rather than an empty value
