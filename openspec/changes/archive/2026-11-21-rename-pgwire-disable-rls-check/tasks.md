## 1. Specification
- [x] Validate this change strictly before implementation.

## 2. Implementation
- [x] Rename the CLI flag and TOML field, rejecting the old names while preserving default and override behavior and the Rust API.
- [x] Update help, example configuration and documentation to describe the full effect and migration.
- [x] Test both configuration forms, defaults, invalid values, old-name rejection and shared configuration handling.

## 3. Verification
- [x] Run relevant tests, formatting, Clippy with warnings denied, workspace tests, rustdoc with warnings denied and strict OpenSpec validation.
