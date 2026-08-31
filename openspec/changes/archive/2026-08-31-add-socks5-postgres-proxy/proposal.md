## Why

PostgreSQL information bases are sometimes reachable only through an SSH
dynamic tunnel or another SOCKS5 gateway. The CLI currently opens only direct
database connections, so users must arrange a separate TCP forward.

## What Changes

- Add an optional `--socks5-proxy HOST:PORT` PostgreSQL connection option to
  both `metadata postgres` and `console postgres` (including the `repl` alias).
- When the option is present, establish the PostgreSQL transport through the
  unauthenticated SOCKS5 proxy and let the proxy resolve the database hostname.
- Validate the proxy endpoint before attempting a network connection and
  report proxy negotiation failures as connection errors.
- Preserve the existing direct connection path when the option is absent.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: allow CLI PostgreSQL connections through an optional SOCKS5
  proxy.

## Impact

- CLI-only connection and help-text change.
- The core `open-sdbl` library and its dependency-free, I/O-free boundary are
  unchanged.
