## ADDED Requirements

### Requirement: Run independent gateway instances under systemd
The project SHALL provide a systemd template unit that runs one gateway process
per named instance as a dedicated service user. Each instance SHALL use its
own `/etc/open-sdbl/instances/<instance>.toml` and MAY use an optional
`/etc/open-sdbl/instances/<instance>.env` systemd environment file. The TOML
file SHALL use the existing gateway configuration and credential rules. The
existing single-instance unit SHALL remain available.

#### Scenario: Two databases on different ports
- **WHEN** an operator configures and starts two named units with distinct
  listen addresses or ports and different backend databases
- **THEN** both gateway processes run concurrently, each listens at its
  configured address and serves its configured database

#### Scenario: Per-instance credentials
- **WHEN** two instances use separate environment files and credential files
- **THEN** each process receives the credentials configured for its own unit
  through existing credential mechanisms, without passwords in TOML or command
  arguments

#### Scenario: Instance failure or stop
- **WHEN** one instance fails to start, including because its listen address
  is already in use, or an operator stops it
- **THEN** the failure is visible for that unit and does not stop another
  running instance

### Requirement: Document multi-instance operation
The project SHALL document installation and operation of the template with two
concrete instances, including their configuration, credentials, security and
individual service management.

#### Scenario: Deploy and manage two instances
- **WHEN** an operator follows the documentation
- **THEN** it describes distinct listeners and databases, instance naming,
  service account and file permissions, TLS, enable, start, stop, restart,
  status and per-instance journal commands
- **AND** it explains shared service account access, the limitations of
  environment variables for secrets and listen address conflicts

#### Scenario: Change one instance's configuration
- **WHEN** an operator changes one instance's TOML or environment file and
  follows the documented restart procedure
- **THEN** that instance starts with the updated settings and the other
  instance continues to run
- **AND** the documentation distinguishes restarting for configuration or
  credential changes from SIGHUP metadata refresh

#### Scenario: Existing deployment
- **WHEN** an operator uses the existing single-instance service and config
- **THEN** the multi-instance deployment does not require migrating it
