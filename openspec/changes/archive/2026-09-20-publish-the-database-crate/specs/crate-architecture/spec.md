## MODIFIED Requirements

### Requirement: Publish the workspace under MIT with aligned versions
Every package manifest in the repository SHALL declare `license = "MIT"`,
the repository SHALL ship the MIT license text naming the copyright holder,
and the `open-sdbl` library, the `open-sdbl-db` library and the
`open-sdbl-cli` application SHALL share the same semantic version so that a
CLI build names the library contract it was built against.

The two libraries — `open-sdbl` and `open-sdbl-db` — SHALL be publishable,
so an application outside this repository can depend on them from a
registry rather than from a checkout. Every dependency of a publishable
package on another package of this workspace SHALL name the shared version
beside the path. The `open-sdbl-cli` application and the fuzz package are
binaries of this repository and SHALL declare `publish = false`.

#### Scenario: Consistent manifests
- **WHEN** the manifests of the core library, the database library, the CLI,
  and the fuzz workspace are inspected
- **THEN** each declares the MIT license and the three workspace packages
  declare the same version

#### Scenario: Breaking library change
- **WHEN** a release changes the public API of `open-sdbl` incompatibly
- **THEN** the shared version number is raised in every package before the
  release is tagged

#### Scenario: The database library may be published
- **WHEN** the manifest of `open-sdbl-db` is inspected
- **THEN** it declares no `publish = false`, and its dependency on
  `open-sdbl` names the shared version as well as the path, so the package
  can be packaged once `open-sdbl` is in the registry it names

#### Scenario: The application stays unpublished
- **WHEN** `cargo publish -p open-sdbl-cli` is attempted
- **THEN** Cargo refuses, because the application declares `publish = false`
