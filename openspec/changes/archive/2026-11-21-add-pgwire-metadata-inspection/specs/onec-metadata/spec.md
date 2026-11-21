## ADDED Requirements

### Requirement: Extract textual module source
The dependency-free SDK SHALL extract textual BSL from the `text` entry of a
1C module container. The caller's limit SHALL bound container documents and
the returned UTF-8 source. Unsupported, malformed, non-text or image-only
containers SHALL fail explicitly; empty textual modules SHALL remain valid.

#### Scenario: Native module source
- **WHEN** a native module container contains a UTF-8 `text` entry
- **THEN** the SDK returns its exact source without its encoding BOM

#### Scenario: Protected module
- **WHEN** a module container has no `text` entry
- **THEN** the SDK reports an error instead of returning an image or empty source

#### Scenario: Bounded module
- **WHEN** the container entry or returned UTF-8 text exceeds the limit
- **THEN** the SDK returns an error without publishing partial source

### Requirement: Extract measured managed-form source
The SDK SHALL extract the textual module from measured managed-form profiles
49 and 50 when their decoded Config resource has the verified outer structure.
Unknown, invalid or oversized profiles SHALL fail rather than publish guessed
source. Extracted text SHALL preserve the platform-exported source.

#### Scenario: Managed form module
- **WHEN** either measured profile is decoded
- **THEN** its module is identical to Designer's export of the same form
