## ADDED Requirements

### Requirement: Escape untrusted data in terminal output
Every CLI output path that prints database-derived or metadata-derived
text SHALL escape control characters and Unicode bidirectional override
characters so that stored data cannot inject terminal escape sequences,
and SHALL bound the number of printed rows and the width of each cell.

#### Scenario: Stored escape sequence
- **WHEN** a query result cell contains an ESC-initiated control
  sequence or an OSC payload
- **THEN** the printed cell shows an escaped textual form and the
  terminal state (screen, title, clipboard) is unaffected

#### Scenario: Oversized result set
- **WHEN** a query returns more rows than the display limit
- **THEN** the CLI prints up to the limit plus a trailer stating how
  many rows were omitted

### Requirement: Secure PostgreSQL transport by default
The CLI SHALL support TLS for PostgreSQL connections with certificate
and hostname verification as the default mode, SHALL honor `PGSSLMODE`
when no flag overrides it, and SHALL refuse plaintext connections unless
the user explicitly opts in.

#### Scenario: Default connection
- **WHEN** the user connects without transport flags
- **THEN** the connection uses TLS with full verification or fails with
  a diagnostic; it never silently falls back to plaintext

#### Scenario: Explicit plaintext opt-in
- **WHEN** the user passes the plaintext mode without the explicit
  insecure opt-in flag
- **THEN** the CLI refuses to connect and names the required flag

### Requirement: Warn when certificate verification is disabled
Disabling MSSQL certificate verification SHALL emit a visible warning on
every use, and the CLI SHALL offer trusting a specific CA file as the
safe alternative for self-signed deployments.

#### Scenario: Trust flag warning
- **WHEN** the user passes the trust-server-certificate flag
- **THEN** a warning naming the risk is written to standard error before
  connecting

### Requirement: Verify read-only semantics on every provider
Every database session SHALL establish provider-enforced read-only
semantics and verify them server-side before executing user queries; a
failed rollback SHALL poison the session instead of leaving an open
transaction in use.

#### Scenario: MSSQL verification
- **WHEN** a query is executed over an MSSQL session
- **THEN** the session has verified server-side that no stale
  transaction is open before the query runs

#### Scenario: Failed rollback
- **WHEN** a rollback after a failed query itself fails
- **THEN** the session is not reused; the CLI reports the state and
  reconnects or exits

### Requirement: Cancel and bound in-flight queries
Query execution SHALL be cancellable from the keyboard without killing
the process, SHALL restore the terminal state on interruption, and SHALL
be bounded by client-side and server-side timeouts.

#### Scenario: Interrupted query
- **WHEN** the user presses Ctrl-C while a query is executing
- **THEN** the in-flight query is cancelled on the server, the terminal
  is restored, and the REPL returns to its prompt

#### Scenario: Stalled server
- **WHEN** the server stops responding after the handshake
- **THEN** the operation fails with a timeout diagnostic instead of
  hanging indefinitely

### Requirement: Handle credentials without lingering copies
The CLI SHALL read password files through a single opened descriptor
(verifying file type, ownership, and permissions on that descriptor),
SHALL zeroize password material it owns after use, SHALL remove
credential environment variables from the process environment once
consumed, and SHALL NOT accept passwords through command-line
arguments.

#### Scenario: Password file indirection
- **WHEN** the password file path is replaced between check and use
- **THEN** the CLI's checks and its read operate on the same opened
  file, so the substitution cannot bypass validation

#### Scenario: No password flag
- **WHEN** the user passes any password-bearing command-line flag
- **THEN** the CLI rejects it and points at the supported credential
  channels

### Requirement: Authenticate to SOCKS5 proxies
The SOCKS5 client SHALL support username/password authentication in
addition to unauthenticated access, and SHALL surface the proxy's reply
code when a connection is refused.

#### Scenario: Authenticated proxy
- **WHEN** the proxy offers only username/password authentication and
  credentials are configured
- **THEN** the tunnel is established using RFC 1929 authentication
