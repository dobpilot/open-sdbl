## MODIFIED Requirements

### Requirement: Require TLS before a cleartext password
The gateway SHALL request a cleartext password only on a TLS stream, on a loopback peer when explicitly permitted, or on any peer when the operator explicitly permits insecure network connections. Otherwise it SHALL refuse with SQLSTATE `28000` before requesting a password. The password SHALL be kept only in zeroized memory and SHALL never be logged.

#### Scenario: Plain connection from the network
- **WHEN** a network client connects without TLS and insecure network connections are not explicitly allowed
- **THEN** the gateway refuses with `28000` before asking for a password

#### Scenario: Explicitly allowed insecure network connection
- **WHEN** insecure network connections are explicitly allowed and a network client connects without TLS
- **THEN** the gateway asks for the password

#### Scenario: Allowed insecure loopback
- **WHEN** insecure loopback is allowed and a loopback client connects without TLS
- **THEN** the gateway asks for the password

### Requirement: Refuse insecure network listening
The CLI SHALL refuse to listen on a non-loopback address without a TLS certificate and key unless insecure network connections are explicitly allowed. It SHALL validate this before connecting to the database.

#### Scenario: Network listen without TLS
- **WHEN** the operator selects a non-loopback listener without TLS or the explicit network opt-in
- **THEN** the command fails with a usage error before connecting

#### Scenario: Network listen with explicit opt-in
- **WHEN** the operator enables insecure network connections and selects a non-loopback listener without TLS
- **THEN** the CLI accepts the configuration
