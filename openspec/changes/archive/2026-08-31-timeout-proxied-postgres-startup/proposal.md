## Why

The SOCKS5 handshake is timeout-bounded, but after the proxy accepts CONNECT
the PostgreSQL startup/authentication future runs without a deadline. A target
that accepts the tunnel and then stops responding leaves the CLI silent and
stuck indefinitely.

## What Changes

- Apply the connection timeout to PostgreSQL startup and authentication over a
  negotiated SOCKS5 stream.
- Report a phase-specific timeout instead of waiting indefinitely.
- Add a deterministic stalled-server regression test.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: bound the complete proxied PostgreSQL connection setup.

## Impact

- CLI-only connection failure behavior.
- Direct PostgreSQL connections and the core library remain unchanged.
