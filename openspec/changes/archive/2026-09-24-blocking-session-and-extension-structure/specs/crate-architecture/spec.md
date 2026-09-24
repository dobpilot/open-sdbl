## ADDED Requirements

### Requirement: Drive one session from synchronous code
`open-sdbl-db` SHALL offer `BlockingDatabaseSession`, a value owning one
single-threaded Tokio runtime and one `DatabaseSession`, with blocking
`connect`, `configuration`, `metadata`, `query`, `query_each` and `close`.
Each blocking operation SHALL drive the asynchronous `DatabaseSession`
operation of the same name and SHALL NOT carry a statement, a provider
type or decoding of its own, so that limits, timeouts, diagnostics,
session poisoning and provider behaviour are those of the asynchronous
session.

Each value SHALL own its own runtime and its own connection, so that
several values opened side by side are independent and closing one leaves
the others usable. The value SHALL be `Send`.

`close` SHALL take the session out of the value. Every operation after it,
a second `close` included, SHALL answer `DbError::SessionClosed`, whether
the first `close` succeeded or failed. An operation called on a thread that
is already driving a Tokio runtime SHALL answer `DbError::AsyncContext`
rather than panic.

The type SHALL NOT know about any consumer, programming language, session
parameter or command line; the package root SHALL re-export it.

#### Scenario: Same answer both ways
- **WHEN** the same artificial provider answers a configuration read, a
  query and a failing query once through the asynchronous path and once
  through the blocking driver
- **THEN** the results are equal and the errors are the same error

#### Scenario: Two sessions
- **WHEN** two blocking sessions are opened and the first is closed
- **THEN** they were two connections, and the second still answers

#### Scenario: A call after close
- **WHEN** any operation, or `close` again, is called after `close`
- **THEN** it answers `DbError::SessionClosed` and does not panic

#### Scenario: Called from asynchronous code
- **WHEN** a blocking operation is called from inside a Tokio runtime
- **THEN** it answers `DbError::AsyncContext` and does not panic

#### Scenario: No second SQL path
- **WHEN** the blocking module is inspected
- **THEN** it names no statement, no provider session and no driver type

### Requirement: Build credentials from owned strings
`Credentials` SHALL offer a constructor taking an owned `Option<String>`
for each of the PostgreSQL, SQL Server and SOCKS5 secrets, and
`EnvironmentSecret` SHALL offer one taking an owned `Option<String>`. Each
SHALL move the string into zeroized memory inside the package, so that a
caller never has to name the zeroizing crate.

`Credentials` and `EnvironmentSecret` SHALL implement `Debug` without ever
printing a secret. `DatabaseConnection` SHALL carry no secret, and no error
the package answers SHALL contain one.

#### Scenario: A caller without zeroize
- **WHEN** a caller builds credentials from `Some(String)` and `None`
- **THEN** the PostgreSQL secret is present, the absent ones are missing,
  and the caller's code names no type of the zeroizing crate

#### Scenario: A password in a debug print
- **WHEN** credentials holding a password are formatted with `{:?}`
- **THEN** the output names which secrets are present and does not
  contain the password

#### Scenario: A password and a refused connection
- **WHEN** a blocking session connects with a password to a port nothing
  listens on
- **THEN** it answers a connection error whose text and debug form do not
  contain the password

### Requirement: Describe each acquired extension
Each extension a whole-configuration read answers SHALL carry, beside its
identity, name, activity, order and resources: its version, the content
hash of its root, whether safe mode is on, whether it is used in a
distributed information base, its scope, the catalog of the classes and
objects its configuration declares, and the object headers of its
descriptors with the explicit link an adopted object carries to the
main-configuration object it adopts.

A value the reader does not decode reliably SHALL be an `Option` and SHALL
be `None` rather than a default: the version when the record carries none,
safe mode until it is measured, the distributed-base flag and the scope
when the base lacks the column, the catalog when the configuration root is
missing or not in the measured shape, and an object's link when it is
nil. The link SHALL be read from the adopted object's own header; the name
a resource has in `ConfigCAS` SHALL NOT be taken as a link. Both providers
SHALL answer the same row type and share every step after it.

#### Scenario: An applied and a not-applied extension
- **WHEN** a base carries one extension it applies and one it does not
- **THEN** both are answered with identity, order, activity `Some(true)`
  and `Some(false)`, and their own content hash

#### Scenario: An adopted catalog
- **WHEN** an extension adopts a catalog of the main configuration by
  identifier
- **THEN** its object list carries that catalog as adopted, with the
  identifier of the main-configuration catalog as its link

#### Scenario: An object of the extension's own
- **WHEN** an extension declares a catalog of its own
- **THEN** that object is answered as own, without a link

#### Scenario: A configuration root that is not understood
- **WHEN** the configuration root resource of an extension does not have
  the measured shape
- **THEN** the extension is answered with no catalog rather than a partial
  one, and everything else as before

#### Scenario: Safe mode
- **WHEN** any extension is read
- **THEN** safe mode is unknown

#### Scenario: A base without the new columns
- **WHEN** `_ExtensionsInfo` has neither `_ExtensionScope` nor
  `_UsedInDistributedInfoBase`
- **THEN** the extensions are read, with scope and distributed-base use
  unknown
