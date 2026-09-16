## ADDED Requirements

### Requirement: Publish the release build as CI artifacts
A continuous-integration run SHALL leave its release build behind: an
archive of the commit's sources and the CLI executable built for
`x86_64-pc-windows-msvc`, both published as artifacts of that run and
named after the version the manifests declare.

The source archive SHALL contain what the repository tracks at that
commit, without build output, and SHALL unpack into a directory named
after the version.

#### Scenario: Take the sources of a run
- **WHEN** a maintainer opens a finished CI run
- **THEN** a `.tar.gz` of the sources is attached, unpacking into a
  version-named directory

#### Scenario: Take the Windows executable
- **WHEN** the same run is opened
- **THEN** the CLI executable built for Windows MSVC is attached, so trying
  it needs no local Rust toolchain
