# pgwire-gateway Specification

## Purpose
A PostgreSQL wire-protocol gateway that lets libpq clients run SDBL against
a 1C base as an authenticated information-base user, compiling every
statement with that user's rights and never forwarding client SQL.

## Requirements

### Requirement: Serve the PostgreSQL protocol for SDBL
The `open-sdbl-pgwire` package SHALL accept client connections on a
caller-provided listener, speak PostgreSQL frontend/backend protocol 3.0
to them, and treat every query text as SDBL except the fixed whitelist of
client chatter. It SHALL serve 1C bases stored in PostgreSQL and in
Microsoft SQL Server through the same client protocol.

#### Scenario: psql runs an SDBL query on a PostgreSQL base
- **WHEN** `psql` connects to the gateway serving a PostgreSQL 1C base and
  sends `ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура`
- **THEN** it receives a `RowDescription` with one `text` column named
  `Наименование`, the rows of the catalog, and `CommandComplete SELECT n`

#### Scenario: The same query on an MSSQL base
- **WHEN** the gateway serves an MSSQL 1C base and receives the same text
- **THEN** the client receives the same column description and rows in the
  PostgreSQL protocol

### Requirement: Never forward client text to the database
The gateway SHALL send to the database only the SELECT statements the
compiler generates and the fixed metadata and rights acquisition
statements. It SHALL NOT forward client text in any mode.

#### Scenario: SQL text is not passed through
- **WHEN** a client sends `SELECT * FROM _Reference42`
- **THEN** the gateway answers an `ErrorResponse` with the SDBL diagnostic
  and sends nothing to the database

### Requirement: Negotiate TLS and protocol version at startup
The gateway SHALL answer `SSLRequest` with `S` and perform a TLS handshake
when it has a TLS configuration, and with `N` otherwise; it SHALL answer
`GSSENCRequest` with `N`. It SHALL accept protocol 3.0, SHALL answer a
higher 3.x minor with `NegotiateProtocolVersion` and continue at 3.0, and
SHALL refuse other versions. It SHALL refuse a `client_encoding` other
than UTF8.

#### Scenario: TLS requested
- **WHEN** a client with `sslmode=require` connects to a gateway with a
  certificate
- **THEN** the handshake succeeds and the startup continues inside TLS

#### Scenario: Unsupported protocol
- **WHEN** a client sends a startup message with protocol 2.0
- **THEN** the gateway sends an `ErrorResponse` and closes the connection

### Requirement: Require TLS before a cleartext password
The gateway SHALL request the password with
`AuthenticationCleartextPassword` only on a TLS stream, or on a loopback
peer when the operator explicitly allowed insecure loopback. Otherwise it
SHALL refuse the connection with SQLSTATE `28000` before asking for a
password. The password SHALL be kept only in zeroized memory and SHALL
never be logged.

#### Scenario: Plain connection from the network
- **WHEN** a client connects without TLS from a non-loopback address
- **THEN** the gateway refuses with `28000` and never asks for a password

#### Scenario: Allowed insecure loopback
- **WHEN** the operator allowed insecure loopback and `psql` connects to
  `127.0.0.1` without TLS
- **THEN** the gateway asks for the password

### Requirement: Authenticate the client as an information-base user
The startup `user` SHALL name the information-base user. The gateway
SHALL permit login only for a primary `MATCH` against that user's
confirmed production `StoredPasswordValue` and enabled standard
authentication, using the `access-rights` contract with secondary mode
`NONE`. Synthetic sources SHALL NOT authenticate production clients.
Supported algorithms SHALL be legacy SHA-1 and four-field IDs 1–3.
There SHALL be no reverse engineering of password fields or H2 fallback.

#### Scenario: Correct password over TLS
- **WHEN** `Петрова (бухгалтер)` has standard authentication enabled and a
  confirmed supported `StoredPasswordValue` associated with her `v8users`
  record, and connects over TLS with a password matching H1
- **THEN** the gateway sends `AuthenticationOk`, the startup
  `ParameterStatus` messages, `BackendKeyData` and `ReadyForQuery`

#### Scenario: Extended supported password profiles
- **WHEN** a user with standard authentication enabled and a confirmed
  production source supplies a password matching H1 under four-field
  ID 1, 2, or 3
- **THEN** each profile permits the same successful startup as legacy SHA-1

### Requirement: Hide verification failures behind one authentication error
Unknown users, disabled standard authentication, and all non-`MATCH`
verification outcomes SHALL receive identical SQLSTATE `28P01` and
message. This SHALL include mismatches, absent passwords, unknown
sources, unsupported profiles, malformed records, and input failures.
The gateway SHALL NOT send internal verification reasons, stored values,
or hashes to the client, and SHALL NOT send `AuthenticationOk` on failure.

#### Scenario: Wrong password and unknown user look the same
- **WHEN** one client sends a wrong password for an existing user and
  another a password for a user that does not exist
- **THEN** both receive `28P01` with the same message

#### Scenario: Unsupported profile and unknown source fail closed
- **WHEN** one user's value has four-field algorithm ID 0 or an unknown ID
  and another user's
  `StoredPasswordValue` source mapping is unconfirmed
- **THEN** both connections receive the same `28P01` message as a wrong
  password, without `AuthenticationOk` or internal verification details

#### Scenario: Uppercase digest grants no authentication
- **WHEN** a candidate matches H2 but not H1 of any confirmed supported value
- **THEN** the gateway refuses with `28P01`

#### Scenario: Absent password is not an empty-password login
- **WHEN** a client sends an empty password but the user's confirmed
  state is explicitly `password_is_set = false` or the stored value is empty
- **THEN** the gateway refuses with `28P01`

#### Scenario: Synthetic source cannot authenticate a production client
- **WHEN** the verifier reports `MATCH` for a value marked
  `synthetic-public-stored-value-legacy` or
  `synthetic-public-stored-value-8.3.27` in a production gateway
- **THEN** the gateway refuses with `28P01`

### Requirement: Derive the session of the authenticated user
After authentication the gateway SHALL derive the user's restrictions and
session parameters as console `\as` does and SHALL combine them with the
session parameters the operator bound to the gateway. The operator's
values SHALL win, and the client SHALL NOT set session parameters.

#### Scenario: Separator value bound by the operator
- **WHEN** the gateway was started with a value for the data separator
  session parameter and a client queries a separated table
- **THEN** the separator predicate uses the operator's value

### Requirement: Compile every query with the authenticated user's rights
Every statement SHALL be prepared in `RestrictionMode::Restricted` with the
user's derived restrictions and SHALL pass the complete field-rights check
of the user before any SQL is generated. A target or field that cannot be
proven readable SHALL refuse the statement with SQLSTATE `42501`.

#### Scenario: Restricted catalog
- **WHEN** a user whose role restricts reading of `Справочник.Контрагенты`
  queries it without `РАЗРЕШЕННЫЕ`
- **THEN** the generated SQL carries the expanded restriction and only the
  permitted rows are returned

#### Scenario: Denied attribute
- **WHEN** the user projects an attribute no role of theirs may read
- **THEN** the client receives `42501` naming the field and the database
  receives no query

### Requirement: Execute Simple Query batches
On a `Query` message the gateway SHALL compile the text as one SDBL batch
with the connection's temporary tables. For each statement that returns
rows it SHALL send `RowDescription`, the `DataRow`s and
`CommandComplete SELECT n`; a statement that only places or destroys a
temporary table SHALL send its `CommandComplete` alone. A query parameter
without a value SHALL fail with `42P02` naming it.

#### Scenario: Temporary table batch
- **WHEN** a client sends `ВЫБРАТЬ … ПОМЕСТИТЬ ВТ ИЗ …; ВЫБРАТЬ * ИЗ ВТ`
- **THEN** it receives a completion for the first statement and the rows
  of the second

#### Scenario: Temporary table kept for the connection
- **WHEN** a later `Query` on the same connection reads `ВТ`
- **THEN** the table is available until `УНИЧТОЖИТЬ ВТ`, `DISCARD ALL` or
  the end of the connection

#### Scenario: Parameter in a simple query
- **WHEN** a simple query references `&Дата`
- **THEN** the client receives `42P02` naming `Дата`

### Requirement: Execute Extended Query messages
The gateway SHALL support `Parse`, `Bind`, `Describe`, `Execute`, `Close`,
`Flush` and `Sync` with named and unnamed statements and portals.
`Execute` with a row limit SHALL suspend the portal with `PortalSuspended`
and resume it on the next `Execute`. After an error the gateway SHALL
discard messages until `Sync`.

#### Scenario: Portal with a row limit
- **WHEN** `Execute` asks for 100 rows of a 250-row result three times
- **THEN** the client receives 100, 100 and 50 rows, the first two ending in
  `PortalSuspended` and the last in `CommandComplete SELECT 50`

#### Scenario: Describe before bind
- **WHEN** a client sends `Describe` for a parsed statement
- **THEN** it receives a `ParameterDescription` with one entry per distinct
  parameter and the `RowDescription` of the result

### Requirement: Bind positional parameters to SDBL names
The parameter `$k` of a statement SHALL be the `k`-th distinct `&Имя` of
its SDBL text in first-occurrence order. A literal `$k` token in the text
SHALL be an SDBL lexing error.

#### Scenario: JDBC prepared statement
- **WHEN** a client parses `ВЫБРАТЬ Ссылка ИЗ Документ.Заказ ГДЕ Дата >= &Начало И Склад = &Склад`
- **THEN** `$1` is `Начало` and `$2` is `Склад`

### Requirement: Decode bound values by declared type
`Bind` SHALL decode each value by its declared type and format: boolean,
integer and numeric types to a number or boolean, character types and
unspecified to a string, `date`/`timestamp`/`timestamptz` to a date,
`bytea` to a reference when 16 bytes long and binary otherwise, `uuid` to
a UUID, and one-dimensional arrays to a list. Other types SHALL fail with
`42804`.

#### Scenario: Date and reference
- **WHEN** the JDBC statement above is bound with a `timestamp` and a
  16-byte `bytea`
- **THEN** it compiles with a date and a reference parameter

#### Scenario: List parameter
- **WHEN** `ГДЕ Код В (&Коды)` is bound with a `text[]` value
- **THEN** the parameter compiles as a list

#### Scenario: Unsupported type
- **WHEN** a value is bound as `jsonb`
- **THEN** the client receives `42804`

### Requirement: Describe result columns natively
The gateway SHALL describe each result column by its `ColumnKind`:
references and binary data as `bytea`, strings as `text` or length-bound
`varchar`, numbers as `numeric` with precision and scale, booleans as
`bool`, dates as `timestamp`, UUIDs as `uuid`. It SHALL send values in the
text or binary format the client asks for per column.

#### Scenario: Reference column in text format
- **WHEN** a client selects `Ссылка` of a catalog in text format
- **THEN** the column type is `bytea` and the value is `\x` followed by 32
  lower-case hex digits

#### Scenario: Number in binary format
- **WHEN** a client asks for binary results of a `numeric(15,2)` resource
- **THEN** `tokio-postgres` decodes the value to the same decimal the
  console prints

### Requirement: Answer client chatter locally
The gateway SHALL answer, without touching the database, an empty query,
`SET`/`RESET`, `SHOW` of its startup parameters and of
`transaction_isolation`, `BEGIN`/`START TRANSACTION [READ ONLY]`,
`COMMIT`/`END`/`ROLLBACK`/`ABORT`, `DISCARD ALL`, `SELECT version()`,
`SELECT current_schema()` and `SELECT 1`. `START TRANSACTION READ WRITE`
SHALL fail with `25006`.

#### Scenario: DBeaver connection test
- **WHEN** a driver sends `SET application_name = 'DBeaver'` and
  `SELECT version()`
- **THEN** both succeed and the version names open-sdbl

### Requirement: Hold a read-only transaction block
Between `BEGIN` and its end all reads of a connection SHALL run in one
read-only database transaction, and `ReadyForQuery` SHALL report `T`, or
`E` after an error, until the block ends.

#### Scenario: Consistent snapshot
- **WHEN** a client sends `BEGIN`, two SDBL queries and `COMMIT`
- **THEN** both queries run inside the same read-only database transaction

### Requirement: Report SDBL diagnostics as PostgreSQL errors
A compilation diagnostic SHALL be reported as an `ErrorResponse` with a
SQLSTATE derived from its kind, the diagnostic message, and the 1-based
character position of its byte offset. A database error SHALL be reported
with `08006` for a lost session, `57014` for a cancelled or timed-out
query and `XX000` otherwise, without physical table names unless the
operator enabled verbose errors.

#### Scenario: Syntax error position
- **WHEN** a client sends `ВЫБРАТЬ ИЗ Справочник.Номенклатура`
- **THEN** the `ErrorResponse` carries `42601` and the character position
  of `ИЗ`, and `psql` underlines it

### Requirement: Cancel a running query
The gateway SHALL send a unique `BackendKeyData` per connection and, on a
`CancelRequest` carrying a matching key, SHALL cancel that connection's
running database query, which then ends with `57014`. A request with an
unknown key SHALL be ignored.

#### Scenario: Ctrl+C in psql
- **WHEN** a long query runs and the user presses Ctrl+C in `psql`
- **THEN** the query ends with `57014` and the connection stays usable

### Requirement: Bound resources of the gateway
The gateway SHALL bound the size of one frontend message, the number of
concurrent connections, the startup and authentication time, the idle time
of a connection, the duration of one query, and the rows a suspended
portal may buffer, failing the offending connection or statement instead
of growing without limit.

#### Scenario: Oversized message
- **WHEN** a client sends a message longer than the configured maximum
- **THEN** the gateway sends `08P01` and closes the connection

### Requirement: Start the gateway from the command line
The CLI SHALL provide `open-sdbl serve postgres` and `open-sdbl serve
mssql` taking the connection options of `console`, a listen address
(default `127.0.0.1:5433`), a TLS certificate and key, repeatable operator
session parameters, and a flag allowing insecure loopback connections. It
SHALL acquire metadata and rights once before accepting connections.

#### Scenario: Start on a PostgreSQL base
- **WHEN** the operator runs `open-sdbl serve postgres --host db --database
  unf --user reader --tls-cert c.pem --tls-key k.pem --listen 0.0.0.0:5433`
- **THEN** the gateway reports that it listens and accepts libpq clients

### Requirement: Refuse insecure network listening
The CLI SHALL refuse to listen on a non-loopback address without a TLS
certificate and key, before connecting to the database.

#### Scenario: Network listen without TLS
- **WHEN** the operator passes `--listen 0.0.0.0:5433` without a certificate
- **THEN** the command fails with a usage error before connecting

### Requirement: Operate the running gateway
The CLI SHALL log connections and failed logins to standard error with the
peer address and user name but never the password, SHALL re-acquire
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
