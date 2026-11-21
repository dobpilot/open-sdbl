# Configure console and metadata from TOML

## Why
Operators need to reuse gateway connection settings for console and metadata commands.

## What Changes
- Accept `--config PATH` for `console`, `repl` and `metadata` using the gateway TOML format.
- Ignore recognized gateway-only settings after validating their types; retain existing credential handling and connection validation.

## Capabilities

### New Capabilities
- `cli-configuration`: shared TOML connection configuration for CLI commands.

## Impact
- CLI parsing, documentation and tests; no database or core library changes.
