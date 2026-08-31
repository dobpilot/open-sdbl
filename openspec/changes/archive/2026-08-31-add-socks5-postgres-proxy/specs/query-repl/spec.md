## ADDED Requirements

### Requirement: Route CLI PostgreSQL connections through an optional SOCKS5 proxy
The `open-sdbl-cli` package SHALL accept `--socks5-proxy HOST:PORT` for
`metadata postgres`, `console postgres`, and the `repl` compatibility alias.
When present, the CLI SHALL establish the PostgreSQL byte stream with the
SOCKS5 CONNECT command using the no-authentication method. It SHALL send a
non-IP PostgreSQL host to the proxy as a domain name rather than resolving it
locally. When absent, the CLI SHALL retain its direct PostgreSQL connection
behavior.

#### Scenario: Proxied metadata connection
- **WHEN** `metadata postgres` receives a valid SOCKS5 proxy endpoint
- **THEN** its metadata session uses a SOCKS5 CONNECT tunnel to the requested
  PostgreSQL host and port

#### Scenario: Proxied console connection
- **WHEN** `console postgres` or `repl postgres` receives a valid SOCKS5 proxy
  endpoint
- **THEN** its complete PostgreSQL session uses the negotiated SOCKS5 tunnel

#### Scenario: Proxy-side database name resolution
- **WHEN** the PostgreSQL host is a DNS name and a SOCKS5 proxy is configured
- **THEN** the CONNECT request carries that name in SOCKS5 domain-address form

#### Scenario: Direct connection compatibility
- **WHEN** no SOCKS5 proxy option is provided
- **THEN** the CLI connects directly with the existing PostgreSQL connection
  and authentication options

#### Scenario: Invalid proxy endpoint
- **WHEN** the proxy value lacks a host, lacks a valid nonzero port, or contains
  an unbracketed IPv6 address
- **THEN** the CLI reports a usage error before attempting a connection

#### Scenario: Proxy negotiation failure
- **WHEN** the proxy cannot be reached, requires another authentication method,
  times out, or rejects the CONNECT request
- **THEN** the CLI reports a SOCKS5 connection error without exposing database
  credentials
