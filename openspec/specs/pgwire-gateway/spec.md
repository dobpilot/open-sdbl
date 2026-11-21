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
client chatter, metadata inspection functions, and SQL routed to the virtual
system catalog. It SHALL serve 1C bases stored in PostgreSQL and in Microsoft
SQL Server through the same client protocol.

#### Scenario: psql runs an SDBL query on a PostgreSQL base
- **WHEN** `psql` connects to the gateway serving a PostgreSQL 1C base and
  sends `ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура`
- **THEN** it receives a `RowDescription` with one `text` column named
  `Наименование`, the rows of the catalog, and `CommandComplete SELECT n`

#### Scenario: The same query on an MSSQL base
- **WHEN** the gateway serves an MSSQL 1C base and receives the same text
- **THEN** the client receives the same column description and rows in the
  PostgreSQL protocol

#### Scenario: Catalog SQL is answered by the virtual catalog
- **WHEN** a client sends `SELECT nspname FROM pg_catalog.pg_namespace`
- **THEN** it receives the schemas of the virtual catalog

### Requirement: Never forward client text to the database
The gateway SHALL send to the database only the SELECT statements the
compiler generates and the fixed metadata and rights acquisition
statements. It SHALL NOT forward client text in any mode.

#### Scenario: SQL text is not passed through
- **WHEN** a client sends `SELECT * FROM _Reference42`
- **THEN** the gateway answers an `ErrorResponse` with the SDBL diagnostic
  and sends nothing to the database

#### Scenario: Catalog SQL never reaches the database
- **WHEN** a client sends any query the virtual catalog evaluates
- **THEN** the database receives no statement for it

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
The gateway SHALL send `AuthenticationCleartextPassword` over TLS or when
the operator explicitly permits plaintext on loopback or on the network.
It SHALL refuse any other connection with `28000` before asking for a
password. Normal operation SHALL keep passwords in zeroized memory. Logging
a password SHALL require explicit opt-in to sensitive authentication diagnostics.

#### Scenario: Plain connection from the network
- **WHEN** a client connects without TLS from a non-loopback address and insecure network connections are not allowed
- **THEN** the gateway refuses with `28000` and never asks for a password

#### Scenario: Explicitly allowed insecure network connection
- **WHEN** insecure network connections are explicitly allowed and a network client connects without TLS
- **THEN** the gateway asks for the password

#### Scenario: Allowed insecure loopback
- **WHEN** the operator allowed insecure loopback and `psql` connects to
  `127.0.0.1` without TLS
- **THEN** the gateway asks for the password

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
values SHALL win, and the client SHALL NOT set session parameters. The
gateway SHALL bind every conditional separator's condition constant to the
shared data area by default, unless the operator already bound that
condition, so a base that is not split into data areas is queryable.

#### Scenario: Separator value bound by the operator
- **WHEN** the gateway was started with a value for the data separator
  session parameter and a client queries a separated table
- **THEN** the separator predicate uses the operator's value

#### Scenario: Conditional separator defaults to the shared area
- **WHEN** a base separates a common attribute conditionally on a Config
  constant and the operator bound no condition, and a client queries a
  separated table
- **THEN** the query compiles against the shared (empty) data area instead
  of failing with `data separator requires condition constant`

### Requirement: Compile every query with the authenticated user's rights
By default, every SDBL statement SHALL be prepared in
`RestrictionMode::Restricted` with the user's restrictions. The gateway
SHALL reject unproven target or field reads with `42501` before generating
SQL. The explicit `disable_rls_check` override SHALL bypass role, object,
field and row checks after authentication. The authenticated-inspection
policy below SHALL apply to metadata inspection.

#### Scenario: Restricted catalog
- **WHEN** a user whose role restricts reading of `Справочник.Контрагенты`
  queries it without `РАЗРЕШЕННЫЕ`
- **THEN** the generated SQL carries the expanded restriction and only the
  permitted rows are returned

#### Scenario: Denied attribute
- **WHEN** the user projects an attribute no role of theirs may read
- **THEN** the client receives `42501` naming the field and the database
  receives no query

#### Scenario: Explicit unrestricted access
- **WHEN** the override is enabled and an authenticated user reads an
  otherwise denied object or field
- **THEN** the gateway permits the read

#### Scenario: Invalid password in unrestricted mode
- **WHEN** the override is enabled and a client supplies an invalid password
- **THEN** the gateway refuses authentication with `28P01`

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
The gateway SHALL describe top-level references with one statically known
enumeration target and deferred reference presentations as `text`. Other
references and binary data SHALL be `bytea`, strings `text` or length-bound
`varchar`, numbers `numeric` with precision and scale, booleans `bool`,
dates `timestamp`, and UUIDs `uuid`. It SHALL send values in the requested
text or binary format.

#### Scenario: Enumeration reference field
- **WHEN** a client selects an enumeration-typed field in simple or extended
  protocol
- **THEN** its column is `text` and its value is the localized presentation

#### Scenario: Reference column in text format
- **WHEN** a client selects `Ссылка` of a catalog in text format
- **THEN** the column type is `bytea` and the value is `\x` followed by 32
  lower-case hex digits

#### Scenario: Number in binary format
- **WHEN** a client asks for binary results of a `numeric(15,2)` resource
- **THEN** `tokio-postgres` decodes the value to the same decimal the
  console prints

#### Scenario: Deferred reference presentation
- **WHEN** a client selects a deferred reference presentation in simple or extended protocol
- **THEN** the column has PostgreSQL type `text` and the resolved Unicode value is transmitted as UTF-8

### Requirement: Preserve enumeration reference semantics
The gateway SHALL render a directly selected enumeration reference using
its presentation and retain the original reference for SQL evaluation,
`DISTINCT`, UUID and type functions. Resolving enumeration presentations
SHALL retain target access decisions and separator filtering. Null SHALL
remain null and empty or unknown enumeration references SHALL render as
empty text.

#### Scenario: Enumeration UUID and type
- **WHEN** the same enum field is selected together with its UUID and type
- **THEN** its presentation is text while UUID and type retain their existing
  values and types

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
The CLI SHALL provide `open-sdbl serve postgres` and `open-sdbl serve mssql` taking the connection options of `console`, a listen address (default `127.0.0.1:5433`), a TLS certificate and key, repeatable operator session parameters, and a flag allowing insecure loopback connections. It SHALL acquire metadata and rights and SHALL concurrently prebuild the complete virtual catalog once before accepting connections.

#### Scenario: Start on a PostgreSQL base
- **WHEN** the operator runs `open-sdbl serve postgres --host db --database unf --user reader --tls-cert c.pem --tls-key k.pem --listen 0.0.0.0:5433`
- **THEN** the gateway has acquired the complete prebuilt catalog before it reports that it listens and accepts libpq clients

### Requirement: Refuse insecure network listening
The CLI SHALL refuse to listen on a non-loopback address without a TLS certificate and key unless insecure network connections are explicitly allowed. It SHALL validate this before connecting to the database.

#### Scenario: Network listen without TLS
- **WHEN** the operator selects a non-loopback listener without TLS or the explicit network opt-in
- **THEN** the command fails with a usage error before connecting

#### Scenario: Network listen with explicit opt-in
- **WHEN** the operator enables insecure network connections and selects a non-loopback listener without TLS
- **THEN** the CLI accepts the configuration

### Requirement: Operate the running gateway
The CLI SHALL log connections and failed logins to standard error with the
peer address and user name. Passwords and stored values SHALL only be logged
when sensitive authentication diagnostics are explicitly enabled. On Unix,
`SIGHUP` SHALL re-acquire metadata and rights and prebuild the replacement
catalog concurrently. Connections SHALL keep their current snapshot until
their statement ends. The CLI SHALL shut down gracefully on interrupt.

#### Scenario: Failed login logged
- **WHEN** a client fails authentication
- **THEN** standard error names the peer and the user and contains no password

#### Scenario: Metadata refresh
- **WHEN** the operator sends `SIGHUP` after updating the configuration
- **THEN** new statements compile against the re-acquired metadata and use its complete prebuilt catalog

### Requirement: Identify the configured backing database
The virtual `pg_database`, `current_database()`, `current_catalog` and
information-schema catalog names SHALL use the gateway's configured
backing database (CLI `--database`) for both providers. Client startup
database names or defaults SHALL NOT rename or switch that database.

#### Scenario: Client defaults its database to the login
- **WHEN** a client connects with database `EDIEvents` or its login name
  while the gateway serves `--database unf`
- **THEN** all virtual database/catalog names report `unf`

### Requirement: Answer recorded DataGrip bootstrap probes
The gateway SHALL answer the recorded DataGrip SSL-status and rounded
server-start-epoch probes locally, with boolean and numeric result types.
SSL status SHALL describe the actual client transport. Start time SHALL
belong to the gateway instance, remain constant across connections and
metadata refresh, and never require a backend query. Unsupported variants
SHALL retain existing routing rather than receive a fabricated response.

#### Scenario: DataGrip checks SSL
- **WHEN** it executes `select ssl from pg_stat_ssl where pid = pg_backend_pid()`
- **THEN** it receives the connection's TLS status

#### Scenario: DataGrip checks server restart
- **WHEN** it executes `select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time`
- **THEN** it receives the gateway's stable rounded Unix startup epoch

### Requirement: Answer string-literal connection probes locally
The gateway SHALL answer a single SQL SELECT of one string literal, with
an optional column alias, as client chatter in Simple and Extended Query.
It SHALL return the exact decoded string as text without building the
metadata catalog or touching the database. This exception SHALL NOT
accept a FROM clause, expressions, parameters or multiple statements.
Other statements SHALL retain their existing routing and diagnostics.

#### Scenario: JDBC keep-alive probe
- **WHEN** a JDBC client executes `SELECT 'keep alive'`
- **THEN** it receives one text column and the exact value `keep alive`
  followed by `SELECT 1`, without a database statement

#### Scenario: Prepared probe preserves escaping and alias
- **WHEN** a client prepares `SELECT 'I''m alive' AS "KeepAlive"`
- **THEN** Describe reports one text column named `KeepAlive` and execution
  returns `I'm alive` in either text or binary format

#### Scenario: Probe exception does not route a table query
- **WHEN** a client sends `SELECT 'keep alive' FROM _Reference42`
- **THEN** it follows the SDBL path and is not answered as a probe

### Requirement: Configure the gateway from TOML
The CLI SHALL accept `open-sdbl serve --config PATH` with a TOML file specifying the provider and gateway and connection options. It SHALL reject unknown fields, malformed values and additional command-line options. The same validation and security rules as command-line options SHALL apply. Passwords SHALL use existing credential mechanisms, not the configuration file.

#### Scenario: Start from a configuration file
- **WHEN** the operator supplies a valid TOML configuration with provider and connection options
- **THEN** the gateway starts with the corresponding command-line settings

#### Scenario: Reject invalid configuration
- **WHEN** the configuration contains an unknown field or invalid value
- **THEN** the command fails before connecting to the database

### Requirement: Provide a systemd service example
The project SHALL provide a systemd service and documented configuration example for running the gateway as a dedicated user.

#### Scenario: Service startup
- **WHEN** the operator installs the unit and its configuration as documented
- **THEN** systemd starts the gateway using the TOML configuration

### Requirement: Provide a Windows service configuration
The project SHALL provide a Windows service wrapper configuration and installation instructions using the same TOML configuration. The instructions SHALL describe credentials, service account access, logs, restart and stopping behavior.

#### Scenario: Run under Windows service control
- **WHEN** an operator installs the documented wrapper and configuration
- **THEN** the wrapper starts the gateway with its TOML file and handles Windows service control requests

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

### Requirement: Preserve catalog inventory across separator configurations
The prebuilt catalog SHALL describe supported relations and columns regardless of separator session values. Synthetic values used for schema discovery SHALL NOT affect actual statement execution or access decisions.

#### Scenario: Independent separator
- **WHEN** a relation requires a separator value supplied in a session
- **THEN** the prebuilt catalog exposes its permitted columns before any client session exists

### Requirement: Keep the gateway responsive during catalog construction
Asynchronous state acquisition SHALL perform catalog construction outside the async runtime worker driving gateway acceptance.

#### Scenario: Metadata refresh
- **WHEN** catalog construction for a replacement state takes time
- **THEN** the existing gateway can continue accepting connections until the replacement state is published

### Requirement: Use the configured database name for embedded states
Connections SHALL observe the configured backing database name in virtual catalog results even when their state was constructed from caller-supplied parts.

#### Scenario: State built from parts
- **WHEN** an embedded gateway serves a state constructed from parts with a configured database name
- **THEN** current_database and database names in catalog tables use the configured name

### Requirement: Run independent gateway instances under systemd
The project SHALL provide a systemd template unit that runs one gateway process
per named instance as a dedicated service user. Each instance SHALL use its
own `/etc/open-sdbl/instances/<instance>.toml` and MAY use an optional
`/etc/open-sdbl/instances/<instance>.env` systemd environment file. The TOML
file SHALL use the existing gateway configuration and credential rules. The
existing single-instance unit SHALL remain available.

#### Scenario: Two databases on different ports
- **WHEN** an operator configures and starts two named units with distinct
  listen addresses or ports and different backend databases
- **THEN** both gateway processes run concurrently, each listens at its
  configured address and serves its configured database

#### Scenario: Per-instance credentials
- **WHEN** two instances use separate environment files and credential files
- **THEN** each process receives the credentials configured for its own unit
  through existing credential mechanisms, without passwords in TOML or command
  arguments

#### Scenario: Instance failure or stop
- **WHEN** one instance fails to start, including because its listen address
  is already in use, or an operator stops it
- **THEN** the failure is visible for that unit and does not stop another
  running instance

### Requirement: Document multi-instance operation
The project SHALL document installation and operation of the template with two
concrete instances, including their configuration, credentials, security and
individual service management.

#### Scenario: Deploy and manage two instances
- **WHEN** an operator follows the documentation
- **THEN** it describes distinct listeners and databases, instance naming,
  service account and file permissions, TLS, enable, start, stop, restart,
  status and per-instance journal commands
- **AND** it explains shared service account access, the limitations of
  environment variables for secrets and listen address conflicts

#### Scenario: Change one instance's configuration
- **WHEN** an operator changes one instance's TOML or environment file and
  follows the documented restart procedure
- **THEN** that instance starts with the updated settings and the other
  instance continues to run
- **AND** the documentation distinguishes restarting for configuration or
  credential changes from SIGHUP metadata refresh

#### Scenario: Existing deployment
- **WHEN** an operator uses the existing single-instance service and config
- **THEN** the multi-instance deployment does not require migrating it

### Requirement: Name the access override consistently
The CLI SHALL accept the explicit flag `--disable-rls-check` and the TOML
boolean field `disable_rls_check` to enable the existing unrestricted gateway
mode. Both SHALL default to disabled. The former names `--unrestricted-access`
and `unrestricted_access` SHALL be rejected. Help and documentation SHALL
explain that the override bypasses role, object, field and row restrictions,
while authentication remains required.

#### Scenario: Enable the override
- **WHEN** an operator passes `--disable-rls-check` or sets
  `disable_rls_check = true` in gateway TOML
- **THEN** the gateway uses its existing unrestricted access behavior

#### Scenario: Leave checks enabled
- **WHEN** the flag is absent or the TOML field is false
- **THEN** normal role, field and row checks remain enabled

#### Scenario: Reject old and invalid names
- **WHEN** an operator supplies the old CLI flag or old TOML field, or a
  non-boolean `disable_rls_check` value
- **THEN** the command fails with a usage error

#### Scenario: Reuse configuration for another command
- **WHEN** `console`, `repl` or `metadata` reads a valid TOML file containing
  `disable_rls_check`
- **THEN** it validates the field's boolean type and ignores its value

### Requirement: Find configuration metadata paths
The gateway SHALL implement `SELECT * FROM onec_find_metadata()` returning
logical metadata paths of the base configuration as text rows. An optional
text argument SHALL restrict results to paths containing that substring,
without case sensitivity. If declared resources cannot be indexed, the search
SHALL identify the result as incomplete. Only reports and data processors
SHALL have their `Реквизиты` paths listed.

#### Scenario: Nested form module
- **WHEN** a configuration contains the form module
  `Справочник.Номенклатура.Формы.ФормаЭлемента.Модуль.bsl`
- **THEN** the unfiltered function lists that full path

#### Scenario: Substring filter
- **WHEN** the client calls `onec_find_metadata('номенклатура')`
- **THEN** the function returns matching paths regardless of letter case

#### Scenario: Show attribute paths only for reports and data processors
- **WHEN** catalog, document, report and data processor attributes are indexed
- **THEN** both unfiltered search and a filter matching `Реквизиты` omit the
  catalog and document attributes and retain the report and data processor
  attributes

### Requirement: Show textual configuration resources
The gateway SHALL implement `SELECT onec_show_metadata(path)` returning the
textual BSL or XML for the exact logical path. The gateway SHALL reject absent,
ambiguous, binary, malformed and unsupported resources explicitly.

#### Scenario: Read a form module
- **WHEN** the supplied full path identifies a textual form module
- **THEN** the result contains its BSL source as text

#### Scenario: Read a data composition template
- **WHEN** the supplied full path identifies a textual XML data composition template
- **THEN** the result contains its original schema XML stream as text

#### Scenario: Read a processing
- **WHEN** the path identifies a processing with a textual object module
- **THEN** the result contains the BSL object module

#### Scenario: Read a managed form stored in native serialization
- **WHEN** the path identifies a form with a supported textual module and no XML stream
- **THEN** the result contains the form module as BSL

#### Scenario: A binary template
- **WHEN** the supplied path identifies a binary template
- **THEN** the function returns an error and does not return binary as text

### Requirement: Restrict inspection to authenticated connections
Every authenticated information-base user SHALL be allowed to use both
metadata inspection functions independently of data rights and RLS mode.
Unauthenticated clients SHALL NOT access their results.

#### Scenario: Restricted user inspects a module
- **WHEN** an authenticated user without data access invokes the functions
- **THEN** metadata inspection succeeds even when RLS checks are enabled

### Requirement: Serve inspection through both query protocols
The inspection functions SHALL work with simple and extended query messages,
including text bound parameters and portal row limits. The gateway SHALL NOT
forward client SQL or path text as executable SQL to the backing database.

#### Scenario: Bound filter
- **WHEN** a client binds a text value to `SELECT * FROM onec_find_metadata($1)`
- **THEN** the gateway returns the matching paths

### Requirement: Read a 1C object by its navigation link
The gateway SHALL implement `SELECT onec_get_object(link)` for an internal
1C object link of the form `e1cib/data/Kind.Name?ref=<32 hex digits>`.
It SHALL resolve the metadata name in its own information base and use
the reference bytes in their physical 1C order. It SHALL reject malformed
links, unknown objects and types without a queryable object reference.

#### Scenario: Look up a document using its own link
- **WHEN** a client supplies a document's internal navigation link
- **THEN** the gateway reads only the row with that physical reference
  in the resolved document table

#### Scenario: Malformed or external link
- **WHEN** the client supplies a link with an invalid reference, or supplies
  a URL for another information base
- **THEN** the gateway returns an argument error before reading any row

### Requirement: Return one JSON object of the object's attributes
The function SHALL return one `jsonb` column with the main row's queryable
standard and custom attributes keyed by their logical 1C names. Composite
attributes SHALL use their active value. The return value SHALL be SQL NULL
when the object is absent. It SHALL exclude tabular sections.

#### Scenario: Read native values
- **WHEN** the object has text, number, boolean, date and reference attributes
- **THEN** the JSON object contains text, numbers and booleans as their JSON
  types, dates as strings, and references as navigation links when their
  target metadata is known

#### Scenario: Missing object
- **WHEN** the metadata exists but no row matches the link
- **THEN** the result contains one SQL NULL value

### Requirement: Authorize an object lookup as a data query
An object lookup SHALL apply the same object, field, row and data area
checks as an equivalent SDBL query under the current gateway mode. It
SHALL NOT forward the link or SQL call to the backing database as SQL.

#### Scenario: Object or field read is denied
- **WHEN** object or field rights deny the corresponding SDBL read
- **THEN** the function returns the authorization error without attribute data

#### Scenario: Row is filtered by a restriction
- **WHEN** a row or data area restriction excludes the requested object
- **THEN** the result contains SQL NULL, as for a missing row

### Requirement: Bind object links in both query protocols
The function SHALL support string literals in simple queries and a text
argument bound to `$1` in extended queries.

#### Scenario: Prepare a lookup without an object link
- **WHEN** a client prepares `SELECT onec_get_object($1)`
- **THEN** the gateway describes a text argument and a `jsonb` result before
  the link is bound

### Requirement: Follow an attribute path from a 1C object link
The gateway SHALL support `onec_get_attribute_text(link, path)`. Each
non-final path segment SHALL select a typed reference and the final segment
SHALL select a value rendered as text. Each read SHALL use the caller's
object, field, row, and data area rules. It SHALL project only the requested
attribute at each step.

#### Scenario: Resolve the name of a report
- **WHEN** a report variant's `Отчет` refers to a metadata identifier with
  `ПолноеИмя` equal to `Отчет.Запасы`
- **THEN** `onec_get_attribute_text(link, 'Отчет.ПолноеИмя')` returns
  `Отчет.Запасы`

#### Scenario: A linked row is excluded
- **WHEN** a link is empty, a row is absent, or a row filter excludes it
- **THEN** the function returns SQL NULL
- **WHEN** an object or field read is denied
- **THEN** the function returns the access error

### Requirement: Resolve a metadata reference property
The gateway SHALL support `onec_get_metadata_ref(name, property)`,
initially for `ОсновнаяСхемаКомпоновкиДанных` on reports. It SHALL return
the configured template's logical path or SQL NULL for a known unset
property. Unsupported or unresolvable properties SHALL return an error.

#### Scenario: The main schema has a custom name
- **WHEN** a report's configured main schema points to the template
  `ВыбраннаяСхема`
- **THEN** the function returns its path, independently of other template names

### Requirement: Compose text service calls
Service calls SHALL accept bounded nested scalar text service calls in
argument positions in both query protocols. Prepared statements SHALL
describe their positional text arguments and outer result type before
execution. A nested SQL NULL SHALL propagate through dependent calls.

#### Scenario: Load a report variant's main schema
- **WHEN** a client passes the result of the attribute lookup into metadata
  reference resolution and passes that result into `onec_show_metadata`
- **THEN** the gateway returns the configured schema XML in one query

#### Scenario: Null intermediate reference
- **WHEN** a nested lookup returns SQL NULL
- **THEN** its calling function returns SQL NULL without reading its resource

### Requirement: Read query data sets from data composition schema XML
The gateway SHALL support `SELECT * FROM onec_dcs_queries(xml)` for
authenticated clients and return two columns, `name text` and `query text`.
Each data set with a direct `query` element SHALL produce one row in document
order. The gateway SHALL decode the query text from XML, preserve its
whitespace and line breaks, and SHALL NOT execute it.

#### Scenario: Extract queries from a report's main schema
- **WHEN** a client passes `onec_show_metadata(path)` into `onec_dcs_queries`
  and the XML contains several query data sets and a data source
- **THEN** it receives a row per query data set with the corresponding
  name and query text, including any queries inside a union
- **AND** a data set without a name has SQL NULL in the `name` column

#### Scenario: Recognize both XML formats
- **WHEN** the argument is a Designer export or a native schema under
  `SchemaFile` with the 1C schema namespace
- **THEN** the gateway finds the query data sets independently of the XML prefix

#### Scenario: Missing schema or no query data sets
- **WHEN** the argument is SQL NULL or a valid schema has no query data sets
- **THEN** the result has two text columns and zero rows

#### Scenario: Invalid input
- **WHEN** the function receives malformed XML or a document with a DTD
- **THEN** it returns SQLSTATE `2200M`
- **WHEN** a well-formed document has no recognized schema root
- **THEN** it returns SQLSTATE `22023`

### Requirement: Serve data composition query extraction through both protocols
The gateway SHALL permit XML literals and composed text calls in simple
and extended queries, and text parameters in extended queries. It SHALL
describe both text columns before execution and honor the requested
result formats and portal row limits. XML input, the parsed document and
buffered rows SHALL be bounded. An XML argument within the resource size
limit SHALL be allowed to exceed the text path argument limit.

#### Scenario: Prepare with a text parameter
- **WHEN** a client prepares `SELECT * FROM onec_dcs_queries($1)`
- **THEN** the gateway describes a text parameter and both result columns
- **WHEN** the client executes this statement with a valid schema larger
  than the metadata path limit and within XML and protocol limits
- **THEN** it receives the query data sets

#### Scenario: Schema exceeds XML parsing limits
- **WHEN** client XML exceeds the supported depth or another parser limit
- **THEN** the gateway returns SQLSTATE `54000` for the statement
