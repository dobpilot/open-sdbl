## RENAMED Requirements

- FROM: `### Requirement: Verify read-only semantics on every provider`
- TO: `### Requirement: Keep database sessions read-only`

## MODIFIED Requirements

### Requirement: Keep database sessions read-only
Every database session SHALL establish provider-enforced read-only
semantics where the provider offers them, and SHALL recover from a failed
query without leaving an open transaction behind: a failed rollback SHALL
poison the session instead of letting it be reused.

The session SHALL NOT probe the server for its own state before a read —
neither role membership, nor isolation level, nor transaction count. Those
checks guard against writes the CLI cannot make: the compiler generates
`SELECT` statements only, and provisioning the account is the operator's
decision.

#### Scenario: PostgreSQL session
- **WHEN** a query is executed over a PostgreSQL session
- **THEN** it runs inside a `READ COMMITTED READ ONLY` transaction

#### Scenario: MSSQL verification
- **WHEN** a query is executed over an MSSQL session
- **THEN** no verification statement is sent first: the session opens its
  own transaction, reads, and rolls back

#### Scenario: Failed rollback
- **WHEN** a rollback after a failed query itself fails
- **THEN** the session is not reused; the CLI reports the state and
  reconnects or exits

#### Scenario: Login with wider rights
- **WHEN** the login is a member of `db_owner`, or the session runs at an
  isolation level other than read committed
- **THEN** the query runs, because neither changes what the CLI sends
