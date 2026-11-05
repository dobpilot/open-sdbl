# Catalog-introspection corpus

The SQL statements real clients send to learn what a PostgreSQL database
holds. The virtual catalog of the gateway must evaluate every one of them
(see `openspec/changes/add-pgwire-catalog-emulation/specs/pgwire-catalog`).

## Files

One file per client tag, statements in first-seen order separated by a
line `-- next`:

| File | Client | Status |
|---|---|---|
| `psql13.sql` | psql 13 from `postgres:13` (`\dn`, `\dt`, `\dt pattern`, `\dv`, `\d`, `\d+`, `\dT`) | recorded 2026-10-26 |
| `psql18.sql` | psql 18.6 (Fedora) against the same server | recorded 2026-10-26 |
| `pgjdbc.sql` | pgjdbc 42.7.4 `DatabaseMetaData` and a prepared statement | not yet recorded |
| `psycopg.sql` | psycopg 3.2 `TypeInfo.fetch` | not yet recorded |
| `psqlodbc.sql` | psqlODBC (Debian bookworm) `SQLTables`, `SQLColumns`, `SQLPrimaryKeys` | not yet recorded |
| `dbeaver.sql` | DBeaver navigator: connect, expand schemas, open columns | needs a manual session |
| `datagrip-list-schemas.sql` | DataGrip 2025.3 native PostgreSQL `ListSchemas` | extracted from installed resource and replayed 2026-10-27 |
| `datagrip-bootstrap.sql` | DataGrip 2025.3 connection/introspection preflight | current local SQL log, 2026-10-27 15:15–15:16 |
| `datagrip-list-databases.sql` | DataGrip 2025.3 native `ListDatabases` | extracted from the same installed resource |
| `datagrip-pre-schemas.sql` | DataGrip 2025.3 pre-schema introspection, nine statements | current local session 2026-10-27 15:52, connection details removed |
| `pgjdbc-type-info.sql` | pgjdbc 42.7.3 `TypeInfoCache.getSQLTypeQuery(true)` | installed driver bytecode, bind marker changed to `$1` for direct protocol replay |
| `datagrip-schema-objects.sql` | DataGrip post-schema object discovery | 13 statements from the current 2026-10-27 16:35 session |

## How it is recorded

`record.sh` starts `postgres:13` with `log_statement = all` and
`log_line_prefix = '%a|'`, creates a small schema shaped like a 1C base
(`Справочник.Номенклатура` with a comment, a tabular section and a view in
`РегистрНакопления`), runs each client with its own `application_name`,
and `extract.py` turns the server log into these files. The statements
hold catalog queries and the fixture names only — no customer data.

The server reports version 13, as the gateway does, so psql chooses the
same query variants it chooses against the gateway.

## Current verification

The maintainer selected psql for current verification (2026-10-26). Both
recorded files are replayed by `catalog::tests::psql_corpus`, and the
protocol test `psql_catalog_introspection` runs real psql commands over a
loopback listener and an authenticated fake database engine. Its database
journal must remain empty. Both local psql 18.6 and psql 13.23 from
`postgres:13` passed. This does not establish the pending clients' support.

Recorded statement counts: psql13: 21, psql18: 24.

## DataGrip schema-list regression

The following schema-object phase is retained separately: virtual recovery
and transaction probes, event triggers, FDW/server/user mappings, access
methods, extension versions, languages, class comments, ACLs, casts and
dependencies. New object catalogs are typed empty inventories with physical
catalog xmin and regclass identities; no underlying objects, credentials
or foreign-server options are read. This also fixes routing of pg_cast and
pg_depend queries whose PostgreSQL concatenation/string syntax previously
fell through to the SDBL lexer.

pg_available_extension_versions() is an empty table function with declared
OUT columns. array_agg retains NULLs, yields NULL on no input, returns the
declared array type and charges collected values to the allocation budget.
pg_is_in_recovery is false for the virtual gateway, and txid_current is its
snapshot revision, not a physical backend transaction. Actual pgjdbc now
replays both recorded phases with metadata/getObject and then calls
DatabaseMetaData.getTables, asserting that user metadata tables are present.

The next complete observed session requires typed empty pg_locks,
pg_auth_members, pg_timezone_names and pg_timezone_abbrevs, false
rolbypassrls, and xmin on modeled physical catalog tables (including the
empty pg_tablespace). These are virtual stubs; no backend lock, membership,
timezone or tablespace inventory is read. `age(xid)` uses the virtual
snapshot revision and pg_tablespace_location returns NULL. Empty timezone
views include typed interval columns but do not claim interval-value
encoding or a real timezone inventory.

The SQL subset now supports UNION DISTINCT and same-typed INNER/LEFT JOIN
USING, with key validation on empty sources, merged unqualified keys,
PostgreSQL star ordering and original qualified keys. Mixed key types are
explicitly unsupported. The actual JDBC test calls getColumnType,
getColumnTypeName and getObject for every result column, triggering the
previously missing TypeInfoCache queries. Those additionally need the
modeled array_in function symbol, regproc casts and correct array typinput.
No backend function is invoked. The helper query is stored separately and
tested for both xid and aclitem[].

The current DataGrip log revealed failures before ListSchemas: its SSL
probe fell into SDBL, and the rounded startup-time probe failed at the UTC
string literal. `datagrip-bootstrap.sql` retains those exact statements
and the intervening current_database/current_schemas lookup. The two fixed
probes are client chatter answered from actual client TLS status and the
gateway's stable rounded startup epoch, without a catalog build or DB SQL.
Their syntax is deliberately narrow; this does not add general EXTRACT or
AT TIME ZONE support. `ListDatabases` uses the empty pg_shdescription.
All catalog/database names now come from the configured backend, never the
client's database field (which libpq may default to its login). The actual
JDBC runner executes bootstrap, database listing and schema listing in
that order in both query modes, with a deliberately different URL name.

`datagrip-list-schemas.sql` is the exact `ListSchemas` section of
`com/intellij/database/dialects/postgres/introspector/PgIntroQueries.sql`
inside the installed `intellij.database.dialects.postgres.jar` (SHA-256
`6b5679b041961d70eb586204fe120b99f23f57b33523e82eda841fcf4f9b81c4`).
The same SQL occurs in the local historical DataGrip database log. It was
replayed directly against the running gateway and failed with 42703 for
`n.xmin` before this fix. This is a focused extracted/replayed statement,
not a recording of a complete current IDE navigator session.

The regression checks exact result names/types, visible schemas, public
first, owner, NULL descriptions, binary xid and no generated database SQL.
`pg_namespace.xmin` is a process-local unsigned 32-bit snapshot revision
(OID 28), shared by connections using the same gateway state; refresh
allocates a new revision. It is virtual, is not a backing PostgreSQL XID,
and is omitted by `*` like a PostgreSQL system column. Already-bound
portals retain their old rows and revision across refresh. The JDBC
fixture also runs this exact query with the actual pgjdbc 42.7.3.

`regex-pg13.tsv` holds COPY-text output of 18 `(text, pattern,
case_insensitive, match)` controls evaluated on PostgreSQL 13. It includes
anchors, alternation, groups, ranges/negation, repetitions, Unicode names
and escaped dots/dollar signs. The regex test replays every row. COPY
text doubles backslashes; tabs separate the four fields.

The implemented relation shapes are in `src/catalog/relations.rs` and the
function/type registry in `src/catalog/sql/func.rs`. The exact requirements
of JDBC, psycopg, ODBC and DBeaver, including additional columns and SQL
constructs, remain to be derived from their recordings; task 1.3 stays open.

The test model uses the recording's relation OIDs (16386, 16398) so the
recorded describe queries assert real attribute rows rather than merely
succeeding with empty results. Production OIDs are assigned over the full
snapshot before visibility filtering.

`datagrip-full-schema.sql` preserves all 30 distinct schema-stage SQL texts
from the native DataGrip 2025.3.3 full-introspection session on 2026-10-27
20:57 (after removing bootstrap queries already covered above). Repeated
queries for eight schemas are deduplicated. JDBC `?` placeholders remain;
the JDBC test binds schema OIDs and snapshot revision obtained from
ListSchemas. The logged `IN (null)` scope in the initial object-kind query
is intentionally preserved. No credentials or log headers are included.
The corpus includes table/type/routine/operator discovery, constraints,
indexes, ACLs, comments and view/routine sources. The native table query
must return visible rows; success with empty results is insufficient.
The populated protocol test also checks 1000 table/comment targets and
21000 field ACL rows under the unchanged 64 MiB budget. Both JDBC query
modes fetch column type names and objects; the ignored psql regression
replays the same SQL with schema/revision substitutions.
