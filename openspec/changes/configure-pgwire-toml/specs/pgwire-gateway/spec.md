## ADDED Requirements

### Requirement: Configure the gateway from TOML
The CLI SHALL accept `open-sdbl serve --config PATH` with a TOML file specifying the provider and gateway and connection options. It SHALL reject unknown fields, malformed values and additional command-line options. The same validation and security rules as command-line options SHALL apply. Passwords SHALL use existing credential mechanisms, not the configuration file.

#### Scenario: Start from a configuration file
- **WHEN** the operator supplies a valid TOML configuration with provider and connection options
- **THEN** the gateway starts with the corresponding command-line settings

#### Scenario: Reject invalid configuration
- **WHEN** the configuration contains an unknown field or invalid value
- **THEN** the command fails before connecting to the database

### Requirement: Provide a systemd service example
The project SHALL provide a systemd service and documented configuration example for running the gateway as a dedicated user.

#### Scenario: Service startup
- **WHEN** the operator installs the unit and its configuration as documented
- **THEN** systemd starts the gateway using the TOML configuration

### Requirement: Provide a Windows service configuration
The project SHALL provide a Windows service wrapper configuration and installation instructions using the same TOML configuration. The instructions SHALL describe credentials, service account access, logs, restart and stopping behavior.

#### Scenario: Run under Windows service control
- **WHEN** an operator installs the documented wrapper and configuration
- **THEN** the wrapper starts the gateway with its TOML file and handles Windows service control requests
