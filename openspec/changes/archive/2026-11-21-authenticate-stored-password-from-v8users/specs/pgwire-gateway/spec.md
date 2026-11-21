## MODIFIED Requirements

### Requirement: Authenticate the client as an information-base user
The startup `user` SHALL name the information-base user. The gateway
SHALL permit login only for a primary `MATCH` against that user's
`StoredPasswordValue` read from the `v8users` record and enabled standard
authentication, using the `access-rights` contract with secondary mode
`NONE`. Synthetic sources SHALL NOT authenticate production clients.
Supported algorithms SHALL be legacy SHA-1 and four-field IDs 1–3.

#### Scenario: Correct password over TLS
- **WHEN** `Петрова (бухгалтер)` has standard authentication enabled and a
  supported `StoredPasswordValue` in her `v8users` record, and connects
  over TLS with a password matching H1
- **THEN** the gateway sends `AuthenticationOk`, the startup
  `ParameterStatus` messages, `BackendKeyData` and `ReadyForQuery`

#### Scenario: Extended supported password profiles
- **WHEN** a user with standard authentication enabled and a supported
  record supplies a password matching H1 under four-field ID 1, 2, or 3
- **THEN** each profile permits the same successful startup as legacy SHA-1

#### Scenario: Wrong password is refused
- **WHEN** that user connects with any other password
- **THEN** the gateway refuses with `28P01`

#### Scenario: User without a stored password fails closed
- **WHEN** a user whose record carries no stored password at the measured
  position connects with any password
- **THEN** the gateway refuses with `28P01`
