# Allow explicitly configured plaintext gateway connections

## Why
Operators need to connect clients over trusted networks without configuring gateway TLS.

## What Changes
- Add an explicit opt-in for plaintext client connections from any address, including network listeners.
- Preserve TLS-by-default behavior and the existing loopback-only opt-in.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: explicitly allow plaintext network authentication.

## Impact
- Gateway public configuration, CLI, TOML, tests and documentation.
