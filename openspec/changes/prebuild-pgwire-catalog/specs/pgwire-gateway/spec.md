## MODIFIED Requirements

### Requirement: Start the gateway from the command line
The CLI SHALL provide `open-sdbl serve postgres` and `open-sdbl serve mssql` taking the connection options of `console`, a listen address (default `127.0.0.1:5433`), a TLS certificate and key, repeatable operator session parameters, and a flag allowing insecure loopback connections. It SHALL acquire metadata and rights and SHALL concurrently prebuild the complete virtual catalog once before accepting connections.

#### Scenario: Start on a PostgreSQL base
- **WHEN** the operator runs `open-sdbl serve postgres --host db --database unf --user reader --tls-cert c.pem --tls-key k.pem --listen 0.0.0.0:5433`
- **THEN** the gateway has acquired the complete prebuilt catalog before it reports that it listens and accepts libpq clients

### Requirement: Operate the running gateway
The CLI SHALL log connections and failed logins to standard error with the peer address and user name but never the password, SHALL re-acquire metadata and rights and concurrently prebuild the replacement catalog on `SIGHUP` on Unix while connections keep their current snapshot until their statement ends, and SHALL shut down gracefully on interrupt.

#### Scenario: Failed login logged
- **WHEN** a client fails authentication
- **THEN** standard error names the peer and the user and contains no password

#### Scenario: Metadata refresh
- **WHEN** the operator sends `SIGHUP` after updating the configuration
- **THEN** new statements compile against the re-acquired metadata and use its complete prebuilt catalog
