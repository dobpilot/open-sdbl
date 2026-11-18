# Run multiple pgwire gateways with systemd

## Why
An operator may need to expose several 1C databases on one host. The
existing systemd example names a single configuration file, so operating
independent gateways currently requires copying and maintaining separate
unit files.

## What Changes
- Provide a systemd template unit that starts one gateway process per named
  instance, with a separate TOML file and optional environment file for each.
- Document configuring two instances with different listening ports and
  database connections, credentials, service lifecycle, logs and failures.
- Preserve the existing single-instance unit and CLI configuration behavior.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: provide a repeatable systemd deployment for independent
  gateway instances.

## Impact
- Deployment unit examples, documentation and verification; no protocol,
  CLI, configuration format, core library or production dependency changes.
- Builds on the existing `configure-pgwire-toml` change and credential
  mechanisms. This change does not introduce dynamic database routing,
  shared process state or systemd socket activation.
