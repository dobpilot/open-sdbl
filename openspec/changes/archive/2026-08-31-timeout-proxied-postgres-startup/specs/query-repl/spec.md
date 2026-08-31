## ADDED Requirements

### Requirement: Bound proxied PostgreSQL startup
After a SOCKS5 proxy accepts the CONNECT request, the `open-sdbl-cli` package
SHALL apply its connection timeout to PostgreSQL startup and authentication over
the tunneled stream. Expiration SHALL close the incomplete stream and report
that PostgreSQL startup through SOCKS5 timed out. This deadline SHALL NOT alter
the existing direct connection path.

#### Scenario: Silent database target after SOCKS5 CONNECT
- **WHEN** the proxy establishes the requested tunnel but the database endpoint
  returns no PostgreSQL startup or authentication response
- **THEN** the CLI terminates the connection attempt after the configured
  connection timeout with a PostgreSQL-through-SOCKS5 timeout error

#### Scenario: Responsive PostgreSQL startup
- **WHEN** PostgreSQL completes startup and authentication within the deadline
- **THEN** the CLI starts the connection driver and continues metadata loading

#### Scenario: Direct connection compatibility
- **WHEN** no SOCKS5 proxy is configured
- **THEN** the existing `tokio-postgres` direct connection behavior remains in
  effect
