# Allow explicit unrestricted gateway access

## Why
Some deployments rely on a read-only database replica and need all authenticated gateway users to read all supported metadata without configuring 1C roles.

## What Changes
- Add an explicit gateway opt-in in TOML and CLI to bypass role and field restrictions while preserving user/password authentication.
- Expose supported catalog tables and fields and execute compiled SDBL without role-derived restrictions.
- Skip reading role rights in this mode; default behavior remains unchanged.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: opt-in unrestricted access after authentication.
- `pgwire-catalog`: expose supported metadata without rights filtering in that mode.

## Impact
- Gateway authentication-adjacent authorization, catalog, state acquisition, CLI, tests and documentation.
