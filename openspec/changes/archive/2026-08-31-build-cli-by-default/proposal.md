## Why

The workspace root is also the library package, so Cargo currently selects only
`open-sdbl` as a default member. A normal `cargo build --release` therefore
finishes successfully without producing the documented `open-sdbl` executable.

## What Changes

- Make `open-sdbl-cli` the workspace default member.
- Ensure a root-level `cargo build --release` produces
  `target/release/open-sdbl` while still building the root library as the CLI's
  dependency.
- Keep explicit package and whole-workspace builds compatible.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `crate-architecture`: define the CLI application as the default workspace
  build target.

## Impact

- Root-level Cargo default selection changes from the library package to the
  CLI package.
- Package boundaries, public Rust APIs, and dependency ownership are
  unchanged.
