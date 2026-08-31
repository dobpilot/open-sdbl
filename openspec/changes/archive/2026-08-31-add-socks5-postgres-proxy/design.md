## Context

`metadata postgres` and `console postgres` already share connection parsing and
the `PostgresSession` adapter in `open-sdbl-cli`. `tokio-postgres` can run its
protocol over a caller-provided asynchronous byte stream, so SOCKS5 transport
belongs entirely in that application crate.

## Decisions

### Use one explicit proxy endpoint option

`--socks5-proxy HOST:PORT` accepts a DNS hostname, IPv4 address, or bracketed
IPv6 address plus a required nonzero port. Keeping proxy credentials out of the
option avoids exposing secrets in process arguments. This change supports the
SOCKS5 no-authentication method, which covers local SSH dynamic tunnels and
trusted gateways.

### Resolve the database hostname at the proxy

The SOCKS5 CONNECT request carries PostgreSQL `--host` as a domain name when it
is not already an IP address. This allows access to database names resolvable
only from the proxy network and avoids local DNS leakage.

### Keep direct and proxied transports separate

Without the option, retain `tokio-postgres`'s existing direct connect path and
timeout behavior. With the option, open the proxy TCP stream, negotiate SOCKS5,
and hand the stream to `tokio-postgres`. Bound proxy setup with the same
connection timeout used by the direct path.

### Implement the bounded SOCKS5 handshake in the CLI crate

Only the CONNECT command, no-authentication method, and IPv4/IPv6/domain
addresses are needed. A small application-local implementation avoids adding a
general proxy dependency and keeps all networking out of the root library.

## Error handling

Malformed endpoints fail as CLI usage errors before network access. TCP,
timeout, unsupported-authentication, and proxy reply errors identify the
SOCKS5 setup phase while retaining the CLI's database-connection exit status.
