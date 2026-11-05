## ADDED Requirements

### Requirement: Diagnose authentication failures on the server
The gateway SHALL provide optional server-side authentication failure reasons. A separate opt-in SHALL log the supplied password and available stored value on failure. Both options SHALL default off and SHALL NOT change authentication or the uniform client-facing error. Ordinary diagnostics SHALL NOT include secrets.

#### Scenario: Identify unknown user
- **WHEN** authentication diagnostics are enabled and a client supplies an unknown user
- **THEN** the server logs an unknown-user reason and the client receives the usual authentication error

#### Scenario: Identify password mismatch
- **WHEN** diagnostics are enabled and an enabled user supplies a wrong password
- **THEN** the server logs the verification reason without password material

#### Scenario: Diagnostics disabled
- **WHEN** authentication diagnostics are disabled
- **THEN** failure logs contain no detailed authentication reason

#### Scenario: Sensitive diagnostics explicitly enabled
- **WHEN** the operator explicitly enables sensitive authentication diagnostics and a login fails
- **THEN** the server logs the supplied password and available stored value; the client still receives the same error

## MODIFIED Requirements

### Requirement: Require TLS before a cleartext password
The gateway SHALL request the password with
`AuthenticationCleartextPassword` only on a TLS stream, or on a loopback
peer when the operator explicitly allowed insecure loopback, or on any peer
when insecure network connections are explicitly allowed. Otherwise it SHALL refuse the connection with SQLSTATE `28000` before asking for a
password. The password SHALL be kept in zeroized memory during normal operation.
It SHALL only be logged when sensitive authentication diagnostics are explicitly enabled.

#### Scenario: Plain connection from the network
- **WHEN** a client connects without TLS from a non-loopback address and insecure network connections are not allowed
- **THEN** the gateway refuses with `28000` and never asks for a password

#### Scenario: Allowed insecure loopback
- **WHEN** the operator allowed insecure loopback and `psql` connects to
  `127.0.0.1` without TLS
- **THEN** the gateway asks for the password


### Requirement: Operate the running gateway
The CLI SHALL log connections and failed logins to standard error with the
peer address and user name; passwords and stored values SHALL only be logged when sensitive authentication diagnostics are explicitly enabled. It SHALL re-acquire
metadata and rights on `SIGHUP` on Unix while connections keep their
current snapshot until their statement ends, and SHALL shut down
gracefully on interrupt.

#### Scenario: Failed login logged
- **WHEN** a client fails authentication
- **THEN** standard error names the peer and the user and contains no
  password

#### Scenario: Metadata refresh
- **WHEN** the operator sends `SIGHUP` after updating the configuration
- **THEN** new statements compile against the re-acquired metadata
