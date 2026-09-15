## Why

Connecting to SQL Server fails for a login that is not exactly a
`db_datareader` member:

```
unsafe MSSQL session: transaction_count=0, db_datareader_only=false,
isolation_level=2; use a login in db_datareader and not
db_datawriter/db_owner/sysadmin
```

The check refuses sessions the server would serve perfectly well, and it
guards against something this tool cannot do anyway: the compiler
generates SELECT statements only, and the console executes nothing else.
It is the owner's call how the account is provisioned, and the check takes
that call away.

## What Changes

- Stop requiring `db_datareader`-only role membership and a particular
  transaction isolation level before an MSSQL query.
- Keep verifying server-side that no stale transaction is open before a
  query runs, which protects against reusing a session left mid
  transaction.

## Capabilities

### Modified Capabilities

- `query-repl`: an MSSQL session verifies transaction state, not role
  membership.

## Impact

A login with wider rights now connects. The CLI still asks for read-only
application intent, wraps reads in its own transaction, and rolls back;
nothing it generates writes.
