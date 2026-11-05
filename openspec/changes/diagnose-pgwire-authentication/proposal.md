# Diagnose gateway authentication failures

## Why
Operators cannot determine whether a failed login is caused by an unknown user, disabled standard authentication, or password verification.

## What Changes
- Add an opt-in authentication diagnostics flag for serve and TOML.
- Log safe reason codes on the server. A separate explicit sensitive diagnostic opt-in logs the supplied password and stored value on failed login; the client error remains uniform.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: add opt-in server-side authentication diagnostics.
- `access-rights`: expose stored values to explicit sensitive diagnostics.

## Impact
- Gateway events, CLI options, documentation and tests.
