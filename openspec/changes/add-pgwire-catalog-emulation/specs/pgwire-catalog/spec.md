## Purpose
The virtual PostgreSQL system catalog of the pgwire gateway: it lets psql,
DBeaver, JDBC, psycopg and ODBC clients browse the objects of a 1C base the
authenticated user may read, answered from metadata without database I/O.

## ADDED Requirements

### Requirement: Keep readable relations independent of column authorization
An owner relation or virtual view with proven object Read SHALL remain
visible even when all columns have unsupported field-right mappings or
execution compilation needs unavailable context. Unproven columns SHALL
remain absent. Tabular sections without proven child Read, unsupported
preparation and unknown or denied object decisions SHALL remain omitted.

#### Scenario: A readable task has no mapped fields
- **WHEN** object Read is allowed but every projected task field is unproven
- **THEN** its schema and relation are listed with zero visible attributes

#### Scenario: Compiling row access needs additional context
- **WHEN** object Read is allowed but execution compilation fails
- **THEN** the catalog still lists the relation without inventing column permissions

#### Scenario: Tabular-section Read is refused or unproven
- **WHEN** no section column passes the existing complete child-right check
- **THEN** the section remains omitted despite its owner's Read grant

#### Scenario: Object Read is denied
- **WHEN** the selected user cannot read a relation
- **THEN** neither that relation nor its fields are exposed

### Requirement: Complete the recorded DataGrip schema introspection stage
The catalog SHALL evaluate the recorded full schema introspection corpus,
including ordered aggregates, a single projected unnest, NATURAL INNER/LEFT
joins, translate, greatest and typed schema-object stubs, without forwarding SQL.
Empty inventories SHALL remain empty. Safe local WHERE predicates SHALL
filter wide join intermediates and retain only needed source columns within
existing memory/work limits.

#### Scenario: Full schema introspection
- **WHEN** DataGrip executes the recorded schema-object, table and type queries
- **THEN** Describe and fetching results succeed locally and the table-list query returns readable objects

#### Scenario: Wide attribute inventory
- **WHEN** an attribute ACL or composite-type query filters a populated metadata snapshot by schema
- **THEN** unrelated rows are discarded before retaining wide joined rows within existing budgets

### Requirement: Keep table-list equijoins within catalog budgets
Catalog evaluation SHALL use indexed same-typed integer/oid, boolean and
text equality keys from
JOIN ON/USING and necessary WHERE conjuncts of inner/comma joins to avoid
materializing Cartesian intermediates for table metadata queries. Index
memory/work SHALL be budgeted. Remaining predicates, NULL handling, LEFT
JOIN behavior and existing limits SHALL be preserved.

#### Scenario: JDBC lists tables in a populated configuration
- **WHEN** its table query joins namespaces, classes and comments in a
  configuration with thousands of relations and field comments
- **THEN** table enumeration succeeds without raising memory/work limits

### Requirement: Supply schema-object introspection stubs
The virtual catalog SHALL expose typed empty event-trigger, foreign-data-
wrapper/server/user-mapping, language, cast and dependency relations. Their
physical catalog names SHALL resolve through regclass and expose virtual
xmin. The recorded recovery/transaction probes SHALL return false and a
virtual revision; no backend recovery state, transactions or objects are read.

#### Scenario: DataGrip discovers schema objects
- **WHEN** it executes the recorded post-schema event, FDW, language, cast
  and dependency queries
- **THEN** every query and its JDBC result metadata succeeds locally

### Requirement: Describe empty available extension versions
The catalog SQL subset SHALL expose pg_available_extension_versions() as
an empty typed table function and support array_agg with NULL elements,
NULL on empty input, declared array result types and evaluation budgets.
Function arguments and aggregate expressions SHALL be validated even
when the source is empty.

#### Scenario: DataGrip checks extension updates
- **WHEN** its extension query groups available versions with array_agg
- **THEN** Describe reports name/version-based columns and evaluation
  returns no extension inventory without backend calls

### Requirement: Supply bounded catalog stubs for DataGrip
The catalog SHALL expose empty typed pg_locks, pg_auth_members,
pg_timezone_names and pg_timezone_abbrevs, false rolbypassrls, and virtual
xmin on modeled physical catalog tables. It SHALL support the recorded
age(xid) and pg_tablespace_location calls without backend reads or exposing
physical locks, memberships, timezone inventories or filesystem paths.

#### Scenario: Pre-schema metadata loads
- **WHEN** DataGrip loads transaction locks, timezones, roles, memberships
  and tablespaces using the recorded session queries
- **THEN** every query succeeds with typed empty results or the gateway's
  modeled role, and schema enumeration can continue

### Requirement: Evaluate explicit union distinct
The SQL subset SHALL accept UNION DISTINCT with the same duplicate
elimination and whole-union ordering/limits as bare UNION.

#### Scenario: Timezone union
- **WHEN** a client combines timezone names and abbreviations with UNION DISTINCT
- **THEN** the query evaluates and repeated rows are eliminated

### Requirement: Join catalog sources using named columns
INNER and LEFT JOIN USING SHALL support same-typed keys, validate missing
or ambiguous keys even on empty inputs, compare non-NULL keys and expose
one unqualified key. Star output SHALL list keys first followed by other
left/right columns; qualified references SHALL retain both original keys.
Unsupported key-type combinations SHALL fail explicitly.

#### Scenario: JDBC resolves a PostgreSQL type
- **WHEN** pgjdbc issues a catalog metadata lookup using JOIN USING
- **THEN** metadata lookup and fetching schema result objects succeed

#### Scenario: JDBC recognizes array metadata
- **WHEN** pgjdbc compares typinput to `pg_catalog.array_in` cast as regproc
- **THEN** the cast resolves the modeled built-in symbol and array types
  expose that input symbol, without invoking a backend function

#### Scenario: Left join retains unmatched input
- **WHEN** a left row has no matching USING key
- **THEN** its merged key remains available and qualified right fields are NULL

### Requirement: List the virtual database through DataGrip
The virtual catalog SHALL expose an empty `pg_shdescription` with its
PostgreSQL columns so DataGrip's recorded database-list query can join it.
That query SHALL return the configured backing database, its owner and
NULL description without database I/O, before schema enumeration.

#### Scenario: DataGrip enumerates databases
- **WHEN** DataGrip executes ListDatabases joining `pg_database` to
  `pg_shdescription`
- **THEN** it receives the configured database with NULL description

### Requirement: Expose the virtual schema snapshot revision to DataGrip
The virtual `pg_namespace` SHALL expose `xmin` as PostgreSQL `xid` (OID
28), with text and four-byte unsigned binary encoding. Its process-local
revision SHALL remain constant for one gateway state across connections
and users, and change when a new state is acquired. It SHALL require no
database reads, remain absent from wildcard projections and represent the
virtual snapshot rather than a physical database transaction.

#### Scenario: DataGrip lists schemas
- **WHEN** DataGrip executes its recorded `ListSchemas` query selecting
  namespace OID, xmin, name, description and owner
- **THEN** it receives visible schemas with those five correctly typed
  fields and public first, without database SQL

#### Scenario: Refresh changes the revision
- **WHEN** the gateway replaces its metadata state
- **THEN** subsequent schema listings report the new revision while
  operations retaining the old state keep its revision

#### Scenario: Explicit system column and wildcard differ
- **WHEN** a client selects `N.xmin` or `N.*` from `pg_namespace N`
- **THEN** only the explicit projection includes xmin, and a derived table
  that explicitly projects xmin exposes it as an ordinary result column

### Requirement: Map 1C object kinds to schemas
The catalog SHALL list one schema per 1C object kind present in the
metadata snapshot and readable by the user, named as SDBL names the kind
(`Справочник`, `Документ`, `РегистрНакопления`, …), plus `pg_catalog`,
`information_schema` and an empty `public`.

#### Scenario: psql lists schemas
- **WHEN** a user who may read a catalog and a document runs `\dn` in psql
- **THEN** the list holds `Справочник` and `Документ` and no schema for a
  kind the user can read nothing of

#### Scenario: information_schema.schemata
- **WHEN** a client selects `schema_name` from `information_schema.schemata`
- **THEN** it receives the same kind schemas together with `pg_catalog`,
  `information_schema` and `public`

### Requirement: Map 1C objects to relations
Each readable object SHALL be a table of its kind schema named by its
metadata name; each readable tabular section SHALL be a table named
`<Объект>.<ТабличнаяЧасть>`; each virtual table of a readable register
SHALL be a view named `<Регистр>.<ВиртуальнаяТаблица>`. The SDBL text
`<Схема>.<Имя>` SHALL address the same object.

#### Scenario: psql lists tables
- **WHEN** the user runs `\dt Справочник.*`
- **THEN** psql shows `Номенклатура` and `Номенклатура.Товары` with type
  `table` in schema `Справочник`

#### Scenario: Register virtual tables are views
- **WHEN** the user runs `\dv РегистрНакопления.*`
- **THEN** the balance register `ЗамерОстатков` appears as views such as
  `ЗамерОстатков.Остатки` and `ЗамерОстатков.Обороты`

### Requirement: Describe columns with gateway result types
The columns of a relation SHALL be its queryable fields in metadata order,
named as SDBL names them, with the type OID, modifier and
`format_type` text the gateway reports when the same field is selected,
and SHALL be nullable.

#### Scenario: psql describes a catalog
- **WHEN** the user runs `\d Справочник.Номенклатура`
- **THEN** the listing shows `Ссылка` as `bytea`, `Наименование` as
  `character varying(150)` when its length is 150, and a numeric
  attribute as `numeric(15,2)`

#### Scenario: Types agree with query results
- **WHEN** a client reads the column types of a relation from
  `information_schema.columns` and then selects the same fields with SDBL
- **THEN** the `RowDescription` type OIDs equal the catalog's

### Requirement: Expose synonyms as comments
The synonym of an object or field in the language of the snapshot's
default presentation SHALL be its description in `pg_description`, so
`obj_description` and `col_description` return it; an object or field
without a synonym SHALL have no description.

#### Scenario: Column comment
- **WHEN** a client calls `col_description` for the attribute
  `ЕдиницаИзмерения` whose synonym is `Единица измерения`
- **THEN** it receives `Единица измерения`

### Requirement: List only what the user may read
A relation SHALL be listed only when the user's decision for reading it is
not denied, and a column only when the complete field-rights check of the
user proves it readable; a relation that cannot be decided SHALL be
omitted. Hidden objects and fields SHALL be absent from every catalog
relation and function result.

#### Scenario: Denied catalog is invisible
- **WHEN** no role of the user may read `Справочник.Контрагенты`
- **THEN** neither `\dt`, `pg_class`, `information_schema.tables` nor a
  lookup by its name returns it

#### Scenario: Restricted catalog is visible
- **WHEN** the user's roles restrict reading of `Справочник.Контрагенты`
  with a row-level condition
- **THEN** the table is listed; its rows stay filtered when queried

#### Scenario: Unreadable attribute is hidden
- **WHEN** the field check does not prove an attribute readable
- **THEN** the attribute is absent from `\d` and from
  `information_schema.columns`

### Requirement: Keep identifiers stable for a snapshot
Object identifiers of schemas, relations and types SHALL be the same for
every connection and every evaluation against the same metadata snapshot,
SHALL not collide, and built-in types SHALL keep their PostgreSQL OIDs.

#### Scenario: Two connections agree
- **WHEN** two connections of different users read the OID of
  `Справочник.Номенклатура` from `pg_class`
- **THEN** both receive the same OID, and `text` is OID 25 in `pg_type`

### Requirement: Evaluate catalog SQL in memory
The gateway SHALL evaluate a bounded SQL subset — `SELECT` with joins,
subqueries, `UNION`, `CASE`, casts, aggregates, `ORDER BY`,
`LIMIT`/`OFFSET`, PostgreSQL operators and the catalog functions of the
supported clients — over the virtual catalog only, in Simple and Extended
Query, and SHALL send nothing to the database for it.

#### Scenario: Join over the catalog
- **WHEN** a client sends `SELECT c.relname FROM pg_catalog.pg_class c
  JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace WHERE
  n.nspname = 'Справочник' ORDER BY 1`
- **THEN** it receives the readable catalog tables in name order and the
  database receives no statement

#### Scenario: Prepared catalog query with a parameter
- **WHEN** pgjdbc prepares a `pg_type` lookup with `WHERE typname = $1`
  and binds `numeric`
- **THEN** Describe reports the parameter and result types and Execute
  returns the row of OID 1700

#### Scenario: Declared parameter types survive execution
- **WHEN** a client prepares `SELECT $1 FROM pg_namespace LIMIT 1` with
  parameter OID 23 and binds a binary integer
- **THEN** statement Describe, portal Describe and binary Execute agree
  on OID 23, including when the parameter occurs in a subquery or CTE

#### Scenario: Cast preserves the declared input type
- **WHEN** a client prepares `SELECT $1::text FROM pg_namespace LIMIT 1`
  with parameter OID 23
- **THEN** ParameterDescription retains OID 23, Bind decodes an integer,
  and the result is its text representation

#### Scenario: Limit and offset apply to the whole union
- **WHEN** a catalog query combines SELECT branches with `UNION ALL`
  followed by `ORDER BY`, `LIMIT` and `OFFSET`
- **THEN** ordering and slicing apply to the combined rows, including
  chains of three or more branches and unions nested in a subquery

### Requirement: Route only catalog SQL to the catalog
A text SHALL go to the catalog when, read as SQL, it names a catalog
relation, the `pg_catalog` or `information_schema` schema, or a catalog
function, and reads no other relation. A text that names none of them, or
that parses and also reads a non-catalog relation, SHALL be compiled as
SDBL as before.

#### Scenario: SDBL keeps its path
- **WHEN** a client sends `ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура`
- **THEN** it is compiled and executed as SDBL

#### Scenario: Mixed relations are not catalog SQL
- **WHEN** a client sends `SELECT * FROM pg_class, _Reference42`
- **THEN** the text is compiled as SDBL and fails with its SDBL diagnostic

#### Scenario: Malformed catalog SQL fails as catalog SQL
- **WHEN** a client sends `SELECT relname FROM pg_class WHERE`
- **THEN** it receives a catalog `42601` with the position of the end of
  the text, not an SDBL diagnostic

### Requirement: Fail unsupported catalog SQL explicitly
Catalog SQL using an unsupported construct SHALL fail with `0A000`, an
unknown function with `42883`, an unknown catalog column with `42703`,
and SHALL carry the 1-based character position when one is known; it
SHALL never return a silently wrong result.

#### Scenario: Unknown catalog function
- **WHEN** a client calls `pg_catalog.pg_stat_get_live_tuples(c.oid)` in a
  query over `pg_class`
- **THEN** it receives `42883` naming the function and its position

### Requirement: Bound catalog evaluation
Catalog evaluation SHALL be bounded in input length, nesting depth,
intermediate rows, string allocations and work, and SHALL fail with `54001` beyond the
bound rather than exhaust the process.

#### Scenario: Cartesian explosion
- **WHEN** a client cross-joins `pg_attribute` with itself three times
- **THEN** the statement fails with `54001` and the connection stays
  usable

#### Scenario: Concatenation inside a scalar expression exceeds the budget
- **WHEN** `length(concat(...))` would concatenate more than the remaining
  allocation budget, even though its final result is a small integer
- **THEN** evaluation fails with `54001` before allocating the concatenated
  string, and the connection remains usable

### Requirement: Serve the recorded client introspection corpus
Every introspection query of the recorded corpus — psql 13 to 18 for
`\dn`, `\dt`, `\dv`, `\d`, `\d+`, `\dT`; pgjdbc 42.x `DatabaseMetaData`
`getSchemas`, `getTables`, `getColumns`, `getPrimaryKeys`,
`getTypeInfo`; DBeaver's navigator; psycopg 3 type lookup; psqlODBC
catalog functions — SHALL evaluate without error.

#### Scenario: DBeaver navigator
- **WHEN** DBeaver connects and expands the `Справочник` schema
- **THEN** it shows the readable catalogs, and opening one shows its
  columns with their types and comments

#### Scenario: pgjdbc getColumns
- **WHEN** a JDBC client calls `getColumns(null, "Справочник",
  "Номенклатура", "%")`
- **THEN** it receives one row per readable field with `TYPE_NAME`,
  `COLUMN_SIZE` and `REMARKS` from the catalog

### Requirement: Reuse the metadata snapshot loaded at startup
Catalog introspection SHALL use the loaded in-memory metadata snapshot.
Field lookup indexes SHALL be built once during snapshot finalization and
reused across relation compilations and connections. A refresh SHALL use
the new snapshot's indexes. Catalog requests SHALL perform no metadata or
database reads and SHALL retain the complete user-rights checks.

#### Scenario: Many objects with custom attributes
- **WHEN** a user first runs `\dt` against a snapshot with thousands of
  objects and tens of thousands of custom attributes
- **THEN** preparing each relation reuses the finalized field lookup index
  rather than rebuilding a configuration-wide attribute map
- **AND** the database receives no statement for the catalog request

#### Scenario: Refreshed attribute names
- **WHEN** metadata refresh replaces the snapshot with renamed attributes
- **THEN** subsequent introspection uses the new snapshot's attribute names
  and the old snapshot remains valid for operations already using it
