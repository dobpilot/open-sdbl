# Configure the gateway with TOML

## Why
Operators need a durable configuration and a systemd unit for unattended gateway operation.

## What Changes
- Add `open-sdbl serve --config PATH` to read gateway and database options from TOML.
- Provide an example configuration and systemd unit and Windows service wrapper configuration. Credentials remain in the existing environment or pgpass mechanisms.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: configure and run the gateway as a service.

## Impact
- CLI configuration parsing, deployment examples and documentation.
