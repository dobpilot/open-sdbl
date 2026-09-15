## MODIFIED Requirements

### Requirement: Verify read-only semantics on every provider
Every database session SHALL establish provider-enforced read-only
semantics and SHALL verify server-side, before executing user queries,
that no transaction is already open; a failed rollback SHALL poison the
session instead of leaving an open transaction in use. The session SHALL
NOT require the login to hold particular role membership, nor a
particular transaction isolation level: provisioning the account is the
operator's decision, and the compiler generates SELECT statements only.

#### Scenario: MSSQL verification
- **WHEN** a query is executed over an MSSQL session
- **THEN** the session has verified server-side that no stale
  transaction is open before the query runs

#### Scenario: Login with wider rights
- **WHEN** the login is a member of `db_owner`, or the session runs at an
  isolation level other than read committed
- **THEN** the query runs, because neither changes what the CLI sends

#### Scenario: Failed rollback
- **WHEN** a rollback after a failed query itself fails
- **THEN** the session is not reused; the CLI reports the state and
  reconnects or exits
