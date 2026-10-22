## ADDED Requirements

### Requirement: Preserve default extension decoding limits
The extension restructure decoder without an explicit limit SHALL enforce the
same decoded-byte ceiling as the default raw-DEFLATE decoder. The bounded API
SHALL enforce the caller's limit before parsing decoded data.

#### Scenario: Default decompression ceiling
- **WHEN** a compressed restructure exceeds the default decoded-byte ceiling
- **THEN** decoding returns an error without materializing the excess output

#### Scenario: Explicit small ceiling
- **WHEN** compressed or plaintext restructure data exceeds a caller's explicit limit
- **THEN** decoding fails even if the serialization would otherwise be valid
