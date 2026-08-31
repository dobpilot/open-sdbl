## Context

`connect_socks5` bounds TCP setup and SOCKS5 negotiation with
`CONNECTION_TIMEOUT`. Once it returns a stream, `PostgresSession::connect`
awaits `tokio_postgres::Config::connect_raw` directly. That future includes the
PostgreSQL startup packet and authentication exchange and has no internal
deadline for a caller-provided stream.

## Decision

### Bound raw PostgreSQL startup separately

Wrap `Config::connect_raw` in `tokio::time::timeout` using the same connection
timeout. SOCKS5 setup and PostgreSQL startup each receive a bounded window, so
the total setup remains bounded while a slow proxy does not consume the entire
database authentication allowance.

### Identify the stalled phase

Return a database connection error that explicitly names PostgreSQL startup
through SOCKS5 and includes the elapsed deadline. Existing proxy negotiation
errors retain their current diagnostics.

### Test with a silent local peer

Open a loopback TCP connection, accept it without returning PostgreSQL protocol
bytes, and call the raw-connect helper with a short test-only duration. The
test verifies that the future terminates with the phase-specific timeout.
