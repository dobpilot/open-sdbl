## ADDED Requirements

### Requirement: Deliver a query result one row at a time
A database session SHALL offer a way to read a result row by row: each row
is decoded on its own and handed to the caller before the next is read
from the server. The caller SHALL be able to answer each row with
"continue" or "stop", and a "stop" SHALL end the read at once.

After a stop the session SHALL NOT read, decode, or allocate the rows that
follow. The row-collecting form of a query SHALL be defined in terms of
this one and SHALL keep the signature and the behaviour it has today.

#### Scenario: Stop after the rows the caller wanted
- **WHEN** a caller reads a result of many rows and stops after the first
  ten
- **THEN** exactly ten rows are decoded, and the source is not read past
  the point the caller stopped at

#### Scenario: Read to the end
- **WHEN** a caller answers "continue" to every row
- **THEN** it sees every row of the result, in the order the server sent
  them

#### Scenario: Failure of a row
- **WHEN** decoding a row fails, or the caller answers with an error
- **THEN** the read ends with that error and the transaction is ended the
  way a failed query ends it

#### Scenario: The collecting form is unchanged
- **WHEN** an application calls the row-collecting query
- **THEN** it receives the same rows, the same errors, and the same
  transaction behaviour as before

### Requirement: End a SQL Server statement by stopping the read
SQL Server is given no server-side execution limit by this crate —
`SET LOCK_TIMEOUT` bounds lock waiting only — so a statement that runs too
long cannot be ended by waiting. Stopping a streaming read on a SQL Server
session SHALL therefore drop the connection that carries the statement
rather than drain it, which ends the statement on the server. The session
SHALL report itself unusable afterwards, so that a caller reconnects
instead of sending on a stream left between protocol messages.

#### Scenario: Stopping ends the statement
- **WHEN** a caller stops a streaming read of a SQL Server result early
- **THEN** the connection carrying the statement is dropped and the
  session reports that it is no longer usable

#### Scenario: PostgreSQL keeps its session
- **WHEN** a caller stops a streaming read of a PostgreSQL result early
- **THEN** the transaction is ended and the session stays usable, because
  the server already bounds the statement by `statement_timeout`
