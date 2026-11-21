# Configure catalog evaluation memory limit

## Why
Large information bases can exceed the default catalog query allocation budget during introspection.

## What Changes
- Allow operators to configure the catalog evaluation allocation budget in MiB for serve through TOML or CLI.
- Keep the default 64 MiB and preserve other resource limits.

## Capabilities

### Modified Capabilities
- `pgwire-catalog`: configurable allocation budget for catalog query evaluation.

## Impact
- Gateway limits API, catalog evaluator, CLI configuration, tests and documentation.
