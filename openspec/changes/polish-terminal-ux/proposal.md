# Proposal

## Why

The console and help have little visual identity, and gateway warnings can
merge into an unfinished metadata progress line. Operators need diagnostics
that identify the failed operation and separate backing-database transport
from the TLS policy for incoming gateway clients.

## What Changes

- Use a mint accent and terminal-default text across REPL, `--help`,
  `serve`, and `metadata`. The REPL can show a branch, sphere, or mountain
  in terminal stderr during initialization. Procedural geometry follows the
  window width, with grayscale shading and a green glint. Measured work drives the reveal.
  The prompt takes over as soon as the editor can accept input, including a
  query pasted during startup. Help uses a single-frame title gradient
  with the full usage and a copyable example.
- Show `serve` acquisition on one line with aligned measurements and a
  mint progress edge. Keep warnings and skipped-resource details in stderr
  history, and leave measured summaries on completion or interruption.
- Sweep a mint highlight once over `metadata` in terminal stderr while
  acquisition runs. Print diagnostics as permanent lines and preserve
  successful snapshot stdout byte for byte with either stderr destination.
- Give explicit help, argument errors, and execution failures distinct output
  and exit behavior. Carry PostgreSQL error information to the local
  diagnostic and keep recoverable session failures local to the session.
- Use `--tls-mode` for backing database TLS on both providers.
  `--tls-mode disable` is an explicit plaintext request. Accept existing
  spellings and config fields, preserve driver defaults, and group shared,
  advanced, compatibility, and listener options in scoped help.
- Apply color and motion according to the destination stream, terminal width,
  and environment. Use `TERM=dumb` for static output and honor `NO_COLOR`
  for monochrome output. Keep the serve interface in ordinary scrollback.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-configuration` defines scoped help, command-level diagnostics, stream
  policy, and the shared backing-database TLS syntax and its TOML form.
- `query-repl` defines the startup scenes and first prompt, the
  stderr-only metadata effect and byte-stable stdout, and provider
  transport and diagnostics.
- `pgwire-gateway` defines permanent operator logs, shared startup and
  refresh progress, and the distinction between startup and session failures.

## Impact

The CLI argument parser, TOML expansion, output, line editor integration,
progress, and gateway event handling change. The database layer carries safe
driver diagnostics, transport policy, and structured metadata warnings. The
gateway passes that context to the CLI. README and deployment examples document
the new syntax. Existing data output, valid legacy transport invocations, and
the default security posture are compatibility constraints. Implementation
uses the current dependencies and keeps terminal I/O outside the core library.
