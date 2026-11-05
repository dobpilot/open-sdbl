## ADDED Requirements

### Requirement: Configure connection commands from TOML
The `console`, `repl`, and `metadata` commands SHALL accept `--config PATH` using the same TOML fields and validation as `serve`. They SHALL use connection options and ignore recognized gateway-only options after checking their types. They SHALL reject unknown fields, malformed values, missing files, and additional command-line arguments. Passwords SHALL use the existing credential mechanisms.

#### Scenario: Reuse gateway configuration
- **WHEN** an operator supplies a valid gateway TOML file to `console` or `metadata`
- **THEN** the command uses its database connection settings and ignores gateway-only options

#### Scenario: Invalid configuration
- **WHEN** the configuration contains unknown fields or invalid connection settings
- **THEN** the command fails before connecting to the database
