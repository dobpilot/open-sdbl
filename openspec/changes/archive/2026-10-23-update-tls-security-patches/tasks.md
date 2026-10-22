- [x] Update compatible TLS dependency floors and the resolved lockfile.
- [x] Run formatting, workspace Clippy, tests, rustdoc, release build and strict OpenSpec validation.
- [x] Verify the resolved TLS versions with the RustSec security audit.

## Evidence
- Resolved versions: rustls 0.23.46, rustls-webpki 0.103.16, webpki-roots 1.0.10.
- `cargo audit --file Cargo.lock`: passed, 196 dependencies scanned.
- Existing TLS feature flags and connection code are unchanged.
- `cargo test --workspace --locked`: 1198 passed, 0 failed, 13 ignored (maintenance and opt-in live database tests).
- Workspace formatting, Clippy with warnings denied, rustdoc with warnings denied, locked release build and strict OpenSpec validation passed.
