## Evidence
- [x] Preserve original descriptor/index bytes and verify packed/re-extracted hashes.
- [x] Load the unchanged descriptor in a new native file base and export its XML.
- [x] Compare native explicitly empty rights serialization and completed selected-user permission oracles.
## Implementation
- [x] Reproduce unread rights through the unchanged database fallback on an isolated PostgreSQL control.
- [x] Add exact-fixture and malformed/unknown-profile regression coverage.
- [x] Recognize only the measured complete no-rights descriptor when `.0` is undeclared.
- [x] Verify existing `.0` precedence, unavailable content and descriptor failures remain conservative.
## Validation
- [x] Run formatting, workspace tests, Clippy, rustdoc and strict OpenSpec validation.
- [x] Record the green database control and evidence provenance without changing API or version.
