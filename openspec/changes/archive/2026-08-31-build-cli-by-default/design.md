## Context

Cargo treats the root package as the implicit default member when a manifest is
both a package and a workspace. In this repository that package is the
dependency-free `open-sdbl` library, while the executable belongs to
`crates/open-sdbl-cli`.

## Decision

### Select the CLI package as the sole default member

Set `workspace.default-members = ["crates/open-sdbl-cli"]`. The CLI depends on
the root library, so the default build continues to compile both packages but
now has an executable artifact. Explicit `cargo build -p open-sdbl` remains
available for library-only consumers, and `cargo build --workspace` retains its
existing whole-workspace meaning.

This is preferable to adding a binary target back to the root package because
it preserves the established I/O and dependency boundary.

## Verification

Build with a clean temporary `CARGO_TARGET_DIR` using `cargo build --release`
without package selection and assert that the release directory contains the
`open-sdbl` executable.
