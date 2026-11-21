# Rename the gateway access override option

## Why
Operators want to configure the existing unrestricted gateway mode using the
name `disable_rls_check` in TOML.

## What Changes
- Rename the TOML field to `disable_rls_check` and its CLI equivalent to
  `--disable-rls-check`. The old names are no longer accepted.
- Preserve the existing behavior: successful authentication is still required,
  while role, object, field and row restrictions are bypassed. The default
  remains restricted. Document this broader effect alongside the new name.
- Keep the Rust gateway library API unchanged.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: rename the operator-facing access override.

## Impact
CLI option and TOML parsing, help, examples, documentation and tests. Existing
configuration files and command lines using the old names must be updated before
restart. Builds on `allow-unrestricted-pgwire-access` and
`configure-console-metadata-toml`.
