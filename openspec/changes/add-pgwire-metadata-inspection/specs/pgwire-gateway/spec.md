## MODIFIED Requirements

### Requirement: Serve the PostgreSQL protocol for SDBL
The `open-sdbl-pgwire` package SHALL accept client connections on a
caller-provided listener, speak PostgreSQL frontend/backend protocol 3.0
to them, and treat every query text as SDBL except the fixed whitelist of
client chatter and metadata inspection functions. It SHALL serve 1C bases stored
in PostgreSQL and in Microsoft SQL Server through the same client protocol.

#### Scenario: psql runs an SDBL query on a PostgreSQL base
- **WHEN** `psql` connects to the gateway serving a PostgreSQL 1C base and
  sends `ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура`
- **THEN** it receives a `RowDescription` with one `text` column named
  `Наименование`, the rows of the catalog, and `CommandComplete SELECT n`

#### Scenario: The same query on an MSSQL base
- **WHEN** the gateway serves an MSSQL 1C base and receives the same text
- **THEN** the client receives the same column description and rows in the
  PostgreSQL protocol

### Requirement: Compile every query with the authenticated user's rights
Every SDBL statement SHALL be prepared in `RestrictionMode::Restricted` with the
user's derived restrictions and SHALL pass the complete field-rights check
of the user before any SQL is generated. A target or field that cannot be
proven readable SHALL refuse the statement with SQLSTATE `42501`. Metadata
inspection functions use the separate authenticated-inspection policy below.

#### Scenario: Restricted catalog
- **WHEN** a user whose role restricts reading of `Справочник.Контрагенты`
  queries it without `РАЗРЕШЕННЫЕ`
- **THEN** the generated SQL carries the expanded restriction and only the
  permitted rows are returned

#### Scenario: Denied attribute
- **WHEN** the user projects an attribute no role of theirs may read
- **THEN** the client receives `42501` naming the field and the database
  receives no query

## ADDED Requirements

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
