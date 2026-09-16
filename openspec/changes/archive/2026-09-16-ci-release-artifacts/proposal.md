## Why

Every CI run proves the workspace builds in release, but nothing of that
build survives the run. Trying a version means cloning the repository and
building it, and on Windows that means installing the toolchain first —
even though CI already has one.

## What Changes

- Package the sources of the commit as a `.tar.gz` and publish it as a run
  artifact.
- Build the CLI for `x86_64-pc-windows-msvc` and publish the executable as
  a run artifact.

## Capabilities

### Modified Capabilities

- `crate-architecture`: a CI run leaves the release build behind as
  artifacts.

## Impact

Two more CI jobs and two artifacts per run. Nothing in the crates changes.
