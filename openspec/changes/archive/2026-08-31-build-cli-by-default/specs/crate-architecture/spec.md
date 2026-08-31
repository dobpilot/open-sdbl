## ADDED Requirements

### Requirement: Build the CLI application by default
The workspace SHALL select `open-sdbl-cli` as its default Cargo member. A Cargo
build invoked from the repository root without `--package` or `--workspace`
SHALL build the CLI application and its `open-sdbl` library dependency while
preserving explicit library-only and whole-workspace build selection.

#### Scenario: Default release build
- **WHEN** a user runs `cargo build --release` from a clean repository checkout
- **THEN** Cargo produces the `target/release/open-sdbl` executable

#### Scenario: Explicit library-only build
- **WHEN** a user runs `cargo build --release --package open-sdbl`
- **THEN** Cargo builds the dependency-free library without requiring the CLI
  application target

#### Scenario: Explicit whole-workspace build
- **WHEN** a user runs `cargo build --release --workspace`
- **THEN** Cargo builds both workspace packages and produces the CLI executable
