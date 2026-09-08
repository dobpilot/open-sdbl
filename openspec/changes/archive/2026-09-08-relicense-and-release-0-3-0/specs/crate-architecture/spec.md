## ADDED Requirements

### Requirement: Publish the workspace under MIT with aligned versions
Every package manifest in the repository SHALL declare `license = "MIT"`,
the repository SHALL ship the MIT license text naming the copyright holder,
and the `open-sdbl` library and `open-sdbl-cli` application SHALL share the
same semantic version so that a CLI build names the library contract it was
built against.

#### Scenario: Consistent manifests
- **WHEN** the manifests of the library, the CLI, and the fuzz workspace are
  inspected
- **THEN** each declares the MIT license and the library and CLI declare the
  same version

#### Scenario: Breaking library change
- **WHEN** a release changes the public API of `open-sdbl` incompatibly
- **THEN** the shared version number is raised in both packages before the
  release is tagged
