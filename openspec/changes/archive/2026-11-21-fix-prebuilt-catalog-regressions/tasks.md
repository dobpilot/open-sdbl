## Implementation
- [x] Correct catalog separator handling and database personalization.
- [x] Offload catalog builds during async acquisition.
- [x] Bound metadata workers and provide a portable fallback.
- [x] Add regression tests and validate the change.

## Validation
- Targeted separator and configured-database regressions pass.
- WebAssembly without threads resolves 255, 256 and 1000 fields.
- Workspace Clippy and rustdoc pass with warnings denied.
- `cargo test --workspace --offline` passes with loopback access enabled.
- Strict validation of this change passes. Full strict OpenSpec validation
  reports pre-existing warnings in other specifications and changes.
- pgwire unit tests pass with loopback access; live-provider testing was not run.
