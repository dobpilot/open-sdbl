## 1. Proxied startup deadline

- [x] 1.1 Wrap raw PostgreSQL startup in the existing connection timeout.
- [x] 1.2 Return a phase-specific timeout diagnostic and close the stalled
  stream by cancelling the startup future.
- [x] 1.3 Add deterministic regression coverage with a silent local peer.

## 2. Verification

- [x] 2.1 Rebuild the release CLI and repeat the reported proxied connection.
- [x] 2.2 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, release artifact verification, and strict OpenSpec
  validation.
