## Decision
Use compatible upstream security patches rather than suppressing audit
findings. Keep the existing rustls features and TLS configuration. Raise direct
dependency floors so a fresh resolution cannot select the known affected
rustls or bundled-root versions. Pin the compatible patched verifier in the
workspace lockfile and inspect its resolved dependency chain.

## Validation
Run formatting, workspace Clippy, tests, rustdoc, release build and strict
OpenSpec validation. Repeat GitHub Actions including its RustSec audit and
Windows checks before publishing. Existing TLS tests cover configuration
behavior; upstream security fixes are validated by the resolved versions and
the audit rather than duplicating upstream cryptographic tests.
