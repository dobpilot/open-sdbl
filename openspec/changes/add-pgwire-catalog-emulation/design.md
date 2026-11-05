# Design: virtual PostgreSQL system catalog for the pgwire gateway

## Schema-object stubs and populated table enumeration

Record the thirteen next-stage IDE statements, including empty event/FDW/
language/cast/dependency catalogs. Register their class identities before
regclass resolution; unknown cast/dependency names previously sent valid
PostgreSQL syntax into the SDBL path. Recovery is false and txid_current is
the virtual revision. The extension-version SRF has typed OUT columns and
no rows. Its aggregate supports NULL/empty semantics with allocation charges.

Native getTables then revealed an allocation-budget failure from namespace
cross-products and nested comment joins. Extract necessary equality keys
from ON/USING or AND terms in the WHERE of inner/comma joins. Build a
budgeted right-side index for compatible integer/oid, boolean and text
values. Preserve right-row order, retain all remaining predicates and use
fallback scanning for noncanonical runtime values. Do not index OR terms
or use WHERE keys to change a LEFT JOIN. Empty left inputs skip index
construction. Existing row/work/allocation limits remain in force.

## DataGrip catalog stubs and type metadata follow-up

Retain all nine pre-schema statements from the current IDE log and replay
JDBC column-type, type-name and object getters, not just executeQuery.
The latter trigger TypeInfoCache SQL with JOIN USING and an array_in
regproc cast. Add the required empty typed lock/membership/timezone views,
false rolbypassrls and uniform hidden xmin on modeled physical catalogs.
age uses the virtual revision; tablespace location stays NULL. No backend
inventory or physical filesystem paths are read.

Plan and evaluation share USING key validation and output-column ordering.
Keep physical row slots for qualified references, hide only duplicate
unqualified keys, and sort star slots by merged-key order. Explicit result
projections become ordinary derived-table columns. Support same-typed
INNER/LEFT keys, exclude NULL matches and reject ambiguous/missing keys or
ON plus USING. UNION DISTINCT uses the existing UNION duplicate elimination.
The array_in symbol is catalog metadata only; no function is executed.

## DataGrip bootstrap and database identity follow-up

The current IDE SQL log establishes failures before schema enumeration:
`pg_stat_ssl`/`pg_backend_pid()` fell into SDBL, as did the rounded epoch
expression over `pg_postmaster_start_time()`. Whitelist only those complete
token sequences as local chatter; values come from actual client TLS and
the gateway instance's rounded startup epoch. Numeric/boolean Describe and
Execute agree in both formats. Do not add a general EXTRACT/timezone SQL
grammar or forward the probes. Metadata refresh retains instance start time.

The configured connection's database name is copied into Shared before
moving its PostgreSQL/MSSQL connection into DbEngine. Every client model
uses that name; the startup database is an accepted alias, not an identity
or routing selector. This prevents libpq's login-name default from renaming
pg_database and aligns all information-schema catalog names. Add the empty
shared-description relation needed by native ListDatabases. Replay the
recorded bootstrap before database/schema listings in the actual JDBC test.

## DataGrip schema-list follow-up (2026-10-27)

The installed native PostgreSQL `ListSchemas` query reads namespace xmin.
The current gateway reproduces 42703 for that field, while a direct
nspname listing succeeds. Preserve the exact SQL as a fixture with its
installed-resource provenance, without claiming a complete current IDE
trace. Add xmin to the virtual namespace source as a system column, not
to star projections; explicit subquery projections become normal columns.

Use PostgreSQL xid OID 28, unsigned four-byte binary encoding and a
process-local revision allocated once per GatewayState (special values
0..2 reserved, wrap at the 32-bit boundary). Every user's model for that
state receives the same token. Refresh creates a new state/revision;
already-bound rows keep the old token. The token is not an underlying DB
transaction ID. No metadata or database reads are added to introspection.
Register xid and its array in the existing catalog type system, including
names, lengths, casts and parameter encoding.

## JDBC literal probe follow-up (2026-10-27)

A real pgjdbc 42.7.3 replay reproduces the reported SQLSTATE 42601 and
position 8 for a string-literal keep-alive SELECT. Decode that narrow
statement with the existing bounded PostgreSQL lexer/parser before the
legacy chatter comment/splitting pass. A complete AST must be exactly one
string projection, optional alias, no FROM/filter/group/order/limit/set
operation or extra statement. Answer it through a text chatter result,
without constructing the catalog or invoking the SDBL compiler/database.
All other statement routing stays unchanged. The standard failed-
transaction check applies before answering a probe.

An opt-in real Java driver test covers both protocol modes, isValid,
escaping, labels, prepared/binary text and rollback. It uses an existing
jar and synthetic credentials. The installed jar identity and remaining
IDE/full-introspection limits are recorded in the JDBC fixture README.

## Decisions taken with the maintainer (2026-10-26)

| # | Question | Decision |
|---|----------|----------|
| 1 | How catalog SQL is answered | A bounded SQL subset parsed and evaluated in memory over virtual catalog relations. No canned answers keyed by query text, no forwarding. |
| 2 | Mapping | Schema = 1C object kind by its SDBL name (`Справочник`), table = object (`Номенклатура`), tabular section = `Номенклатура.Товары`, register virtual table = view `ЗамерОстатков.Остатки`. |
| 3 | Visibility | Only what the authenticated user may read, decided as the Restricted compiler decides it. |
| 4 | Clients of the first version | psql (`\dn`, `\dt`, `\dv`, `\d`, `\d+`, `\dT`), DBeaver/pgjdbc `DatabaseMetaData`, `information_schema`, psycopg 3, psqlODBC. |

## Package placement

Everything lives in `crates/open-sdbl-pgwire` under a new private module
tree `catalog/`:

```
catalog/
  model.rs      the per-connection catalog: schemas, relations, columns
  oid.rs        stable identifiers
  relations.rs  pg_catalog / information_schema relations as row sources
  sql/lex.rs    PostgreSQL lexer subset
  sql/parse.rs  SELECT subset → AST
  sql/plan.rs   name resolution, types, parameter inference
  sql/eval.rs   evaluation over row sources, with budgets
  sql/func.rs   functions and operators
  sql/regex.rs  POSIX regular-expression subset for ~, ~*, !~, !~*
```

No new production dependency: the lexer, parser, evaluator and regex
engine are local, like the protocol codec. `open-sdbl` and `open-sdbl-db`
stay unchanged in behaviour; the model is built only from public APIs
(`MetadataSnapshot::objects`, `object_query_name`, `MetadataField::synonyms`,
`QueryCompiler::prepare_with_options`, `user_decisions`,
`check_user_field_access`). If an accessor turns out to be missing, it is
added to the core as a read-only, I/O-free function with its own rustdoc
and test.

## Building the model

The model is built lazily on the first catalog query of a connection and
cached together with the `Arc<GatewayState>` it was built from; a refresh
of the gateway state rebuilds it on the next catalog query.

1. **Relations.** For every object of the snapshot with a query name
   (`object_query_name`), the kind schema is the part before the first dot
   and the relation name the rest. Tabular sections come from the object's
   sections; register virtual tables from the virtual tables the compiler
   supports for that register kind (accumulation: `Остатки`, `Обороты`,
   `ОстаткиИОбороты`; information: `СрезПервых`, `СрезПоследних`;
   accounting: the kinds `accounting-register-virtual-tables` compiles).
2. **Columns.** Each relation is compiled as
   `ВЫБРАТЬ * ИЗ <Вид>.<Имя>` (virtual tables with empty parameters) in
   `RestrictionMode::Restricted` with the user's decisions. The compiled
   columns, minus service columns, give names, order and `ColumnKind`;
   the type mapping is the same function the result path uses, so catalog
   types equal `RowDescription` types by construction.
3. **Visibility.** The restriction request of each compilation goes
   through `user_decisions`; a denied or undecidable target drops the
   relation. Each `FieldUse` of the prepared statement is checked alone
   with `check_user_field_access`; a column whose origin fails is dropped.
   A relation left without columns is dropped. Compilation failures drop
   the relation and are counted, never surfaced as catalog errors.
4. **Comments.** Object and field synonyms in the snapshot's default
   presentation language (`ru` first, as the console's presentation
   plans) become `pg_description` rows.

Building costs one in-memory compilation per relation. For a large
configuration (≈ 5–10 k relations) the build is measured in the tests
with a synthetic snapshot and must stay within a stated bound (target:
under one second in release mode); if it does not, relations are built
per schema on demand.

## Stable identifiers

- Built-in catalog objects (`pg_catalog`, `information_schema`, `public`,
  built-in types, catalog relations, functions) use their PostgreSQL 13
  OIDs.
- User schemas and relations are numbered from 16384 in the sorted order
  of `(kind schema, relation name)` over the **whole** snapshot, not the
  user's visible subset, so every user and every connection sees the same
  OID for the same relation of the same snapshot. Composite row types get
  `reltype` OIDs from the same sequence.
- `attnum` is the column's 1-based position in the compiled `*` list.

## Catalog relations

Columns follow PostgreSQL 13. A column the model cannot fill truthfully
is present with its documented neutral value (`relpages = 0`,
`reltuples = -1`, `relacl = NULL`, `relowner` = the gateway role) rather
than absent, because clients select them unconditionally.

| Relation | Content |
|---|---|
| `pg_namespace` | built-in schemas + visible kind schemas |
| `pg_class` | visible tables (`r`), views (`v`); no indexes, sequences |
| `pg_attribute` | visible columns; no system columns |
| `pg_type` | built-in types the gateway reports, their arrays, one composite per relation |
| `pg_description` | synonyms |
| `pg_database` | one database, named by the configured backend (`--database`), independent of the client startup database |
| `pg_shdescription` | empty shared-object descriptions; the virtual database has no description |
| `pg_roles`, `pg_user` | one role: the authenticated user name |
| `pg_settings` | the startup parameters and `search_path` |
| `pg_index`, `pg_constraint`, `pg_attrdef`, `pg_inherits`, `pg_trigger`, `pg_rewrite`, `pg_policy`, `pg_statistic_ext`, `pg_publication*`, `pg_foreign_table`, `pg_partitioned_table`, `pg_am`, `pg_collation`, `pg_extension`, `pg_proc` (catalog functions only), `pg_tablespace`, `pg_enum`, `pg_range` | the rows clients need, mostly empty |
| `information_schema.schemata`, `tables`, `columns`, `views`, `table_constraints`, `key_column_usage`, `routines`, `parameters` | derived from the above |

The exact relation and column list is fixed by the recorded corpus
(below): a relation or column appears because a supported client reads
it.

`search_path` is reported as `"$user", public, <all kind schemas>` and
`pg_table_is_visible` is true for every listed relation, so `\dt` without
a pattern lists all readable tables. This differs from PostgreSQL when
two kinds hold relations of the same name; it is documented.

## SQL subset

**Lexer:** PostgreSQL identifiers (quoted with `""`, case-folded when
unquoted), strings `'…'`, `E'…'`, dollar-quoted `$$…$$`, numbers,
`$n` parameters, operators, `::`, comments `--` and `/* */`.

**Grammar:** `SELECT [DISTINCT] … FROM … [JOIN … ON|USING] [WHERE]
[GROUP BY] [HAVING] [ORDER BY … [NULLS FIRST|LAST]] [LIMIT] [OFFSET]`,
`UNION [ALL]`, `WITH` (non-recursive), derived tables, scalar, `IN`,
`EXISTS` and correlated subqueries, `CASE`, `COALESCE`, `NULLIF`,
`CAST`/`::`, `IS [NOT] NULL|DISTINCT FROM|TRUE|FALSE`, `BETWEEN`, `LIKE`,
`ILIKE`, `~`, `~*`, `!~`, `!~*`, `OPERATOR(pg_catalog.~)`, `ANY`/`ALL`
over arrays and subqueries, array constructors and subscripts,
`COLLATE` (accepted, ignored), window `row_number() OVER (…)` if the
corpus needs it. `SELECT` without `FROM` is supported.

**Types:** `bool`, `int2/4/8`, `oid`, `numeric`, `text`, `name`, `"char"`,
`varchar`, `regclass`, `regtype`, `regproc`, `regnamespace`, `int2vector`,
`oidvector`, one-dimensional arrays, `aclitem` as text. Casts between
them follow PostgreSQL (`'pg_class'::regclass` resolves the name in the
virtual catalog).

**Functions:** exactly the set the corpus calls — e.g.
`pg_table_is_visible`, `pg_type_is_visible`, `pg_function_is_visible`,
`pg_get_userbyid`, `format_type`, `obj_description`, `col_description`,
`shobj_description`, `pg_get_expr`, `pg_get_indexdef`,
`pg_get_constraintdef`, `pg_get_viewdef` (returns the SDBL text of the
virtual table), `pg_relation_size` / `pg_total_relation_size` (0),
`has_table_privilege` / `has_schema_privilege` (true for listed
objects), `current_schema(s)`, `current_database`, `current_user`,
`session_user`, `version`, `quote_ident`, `array_upper`, `array_length`,
`unnest`, `generate_series`, `information_schema._pg_expandarray`,
`string_agg`, `count`, `min`, `max`, `lower`, `upper`, `substring`,
`concat`, `left`, `position`, `coalesce`. Any other function is `42883`.

**Parameters:** in Extended Query, a `$n` gets its type from its
context (comparison with a typed column, cast) or the declared OID, and
`text` otherwise; Describe returns those types and the result row
description without evaluating.

**Regex:** a backtracking matcher for anchors, `.`, bracket expressions
with ranges and negation, `*`, `+`, `?`, `{m,n}`, groups, alternation and
the escapes psql emits (`\\.`, `\\$`); case-insensitive variants. psql
builds `^(pattern)$` from `\dt` patterns, so this subset covers it.
Backtracking is bounded by the work budget.

**Errors:** syntax → `42601` with character position; unknown relation →
`42P01`; unknown column → `42703`; unknown function → `42883`;
unsupported construct → `0A000`; division by zero and similar → the
PostgreSQL code; budgets → `54001`.

**Budgets:** text ≤ 1 MiB (the message limit already applies), nesting
≤ 32, rows produced by any operator ≤ 1,000,000, total evaluation steps
≤ 50,000,000, cumulative value/regex allocation ≤ 64 MiB. Exceeding fails the statement; the connection survives.

## Routing in the connection

After the chatter whitelist, the text is lexed as SQL. If any identifier
token names `pg_catalog`, `information_schema`, a catalog relation (bare
or qualified) or a catalog function, the text is *catalog-intended*:

- parsed; a parse failure is a catalog `42601` / `0A000`;
- if the parsed statement reads any relation that is not a catalog
  relation, the text goes to the SDBL path unchanged (so
  `SELECT * FROM pg_class, _Reference42` keeps its SDBL diagnostic);
- otherwise it is evaluated by the catalog.

Texts that are not catalog-intended go to SDBL exactly as now; SDBL text
never lexes as catalog-intended in practice because SDBL objects are not
named like catalog relations, and an SDBL text naming one would fail as
SDBL anyway.

The chatter answers stay as they are (`SELECT version()`,
`current_schema()`, `SHOW`); `SHOW search_path` gains the value above.

Catalog statements are allowed inside a read-only transaction block and
do not touch the database session; they are answered even when the
database session is lost.

## Result encoding

Text format for every type; binary format for `bool`, `int2/4/8`, `oid`,
`text`, `name`, `varchar`, `"char"`, `numeric` (existing encoder), and
one-dimensional arrays of those. A binary request for `regclass`,
`aclitem` and the vector types falls back to an error `0A000` naming the
column — no client of the corpus asks for it; this is verified by the
corpus tests with tokio-postgres (which asks binary for all).

## Recorded client corpus

The corpus is the acceptance oracle. It is recorded once, committed as
fixtures under `crates/open-sdbl-pgwire/tests/fixtures/catalog-corpus/`
with a README stating client versions and how it was recorded, and
replayed in tests:

- Start `postgres:13` in a container with `log_statement = all` and a
  small schema with two schemas, a table with a commented column and a
  view.
- Run psql 13 (in the container) and the local psql 18 against it:
  `\dn`, `\dt`, `\dt pattern`, `\dv`, `\d table`, `\d+ table`, `\dT`.
- Run a pgjdbc 42.7.x program calling `getSchemas`, `getTables`,
  `getColumns`, `getPrimaryKeys`, `getTypeInfo`, plus a prepared
  statement (type cache queries).
- Run psycopg 3 (`TypeInfo.fetch`) and psqlODBC (`SQLTables`,
  `SQLColumns`) in a container.
- DBeaver: the maintainer connects once, expands schemas, opens a table's
  columns tab; its statements come from the same server log.
- Extract every statement of the log, deduplicate, keep the text and the
  client it came from. These texts contain no customer data.

Replay test: each corpus statement, with any `$n` bound to a value from
the fixture, evaluates against a fixture snapshot without error; selected
statements also assert results (psql `\d` of a known catalog shows the
expected columns and types).

End-to-end: `tokio-postgres` runs corpus statements through the gateway
with the fake engine; the fake database journal stays empty. Ignored
live tests run real psql (`\dt`, `\d`) against `open-sdbl serve`.

## Security

- Visibility uses the same decisions and field checks as statement
  compilation, so the catalog cannot reveal an object or field a query
  could not read; it does reveal *that* a restricted object exists, as
  the platform's metadata does.
- Synonyms and names come from Config, which a user with read access to
  the object already sees through the platform.
- The evaluator reads only the in-memory model; it has no file, network
  or database access, and is budgeted.

## Testing strategy

- Lexer/parser unit tests and a fuzz target `pgwire_catalog_sql` feeding
  arbitrary text to lex → parse → evaluate on a small model.
- Regex tests against PostgreSQL semantics for the supported subset
  (cases cross-checked on the recording server).
- Model tests: schemas, relation names, columns and types for each object
  kind of the existing fixtures; visibility for denied object, restricted
  object, denied field; OID stability across users and builds.
- Corpus replay and end-to-end tests as above.
- A model-build timing test on a synthetic snapshot of 10,000 relations
  (release-mode bound documented, debug bound generous).

## Open points to settle during implementation
- Whether `ВЫБРАТЬ *` compiles for every kind the compiler supports; kinds
  where it does not are listed in the README and omitted.
- Exact catalog column sets: fixed by the corpus, not guessed.
- Whether DBeaver needs `pg_index`/`pg_constraint` rows for a primary key
  on `Ссылка` to enable its data editor; first version leaves them empty.

## Implementation measurements (2026-10-26)

The maintainer selected psql for current verification; DBeaver recording
and the other client corpora remain follow-up acceptance work. Existing
psql 13/18 recordings are replayed and both real clients run the protocol
introspection test against a fake database engine with an empty journal.

The synthetic 10,000-relation release measurement (candidate identities,
restricted preparation/compilation of each relation, and catalog rows)
fell from 51.46 s to 251 ms (509 ms during concurrent checks) after
qualified metadata lookup reused the
snapshot's existing object-name index. Missing and ambiguous lookups keep
the original scan, preserving diagnostic and compatibility behavior.

SQL nesting is capped at 32 before recursive parsing and on constructed
expression trees, including flat operator chains. This keeps debug builds
within the default Rust thread stack. Regex nesting remains bounded at 64.
Projection count (1664), CTE count (64), and cumulative allocation (64 MiB)
are bounded in addition to the planned row and work budgets. Numeric
comparison uses bounded exact decimals; unsupported decimal arithmetic,
non-column GROUP BY keys and projection SRFs fail explicitly. The rest of
the planned SQL/client surface remains subject to the pending corpus work.

## Startup snapshot indexes (live regression)

Live `\dt` profiling found repeated `index_custom_field_names` calls from
both preparation and compilation of every relation. The earlier benchmark
had only ID columns, so it did not exercise configuration-wide attribute
indexing and is not representative of a populated 1C configuration.

Build physical-table/field-number and extension-field-number lookup maps
in `MetadataSnapshot` finalization, alongside the existing metadata indexes.
Store positions into immutable resolved fields, share the maps when cloning
a snapshot, and reuse them for field names, identities and reference targets.
Index separator positions and their first declared storage types as well.
No query-specific permissions or session values enter these shared maps.

Gateway startup also retains the snapshot's unfiltered relation inventory;
connections filter it with their own rights and cache their visible models.
Field comments use the snapshot's attribute-ID index. Verification includes
custom-attribute-heavy snapshots, repeated connections and an empty fake
DB journal, rather than a schema made only of ID columns.

The populated protocol regression runs first and repeated introspection on
1,000 relations with 20,000 declared custom attributes and complete field
rights, plus a second connection, and asserts no database statements. The
release measurement after indexing is 283 ms for first introspection and
6 ms repeated. The ID-only 10,000-relation benchmark is retained only as a
narrow control and is superseded for the live regression.

Descriptor identity lookup is also finalized once: owner lookup preserves
the first own descriptor, attribute lookup retains all duplicates in source
order. Field-right checks use these indexes while preserving their existing
ownership, type-evidence and role checks. A snapshot refresh constructs new
indexes; no per-user decision enters shared snapshot data.

## Review corrections

Catalog execution receives the parameter OIDs retained by Parse and carries
them into planning of subqueries, CTEs and array expressions. Describe first
infers unspecified parameters, then derives output columns with those final
types. A cast supplies a type only for an unspecified parameter; an explicit
input OID remains the Bind decoder's type, independently of the result cast.

The parser attaches trailing ORDER BY, LIMIT and OFFSET to the complete
UNION, propagating those clauses through every unparenthesized branch. The
evaluator combines the branches before ordering and slicing. A derived
table retains its own clauses inside its query boundary.

Concat charges retained argument values and its output against the cumulative
allocation budget. A streaming text formatter counts UTF-8 bytes and array
escapes without materializing strings, stopping when the remaining budget
is exceeded. Only after reserving that budget is the output string created;
wrapping concat in length therefore cannot hide its intermediate allocation.
Tests cover the original 72 MB case, cumulative charging, binary parameter
decoding (including NULL and nested queries), and two/three-branch unions.

## Tabular-section field authorization

The field checker previously refused every field carrying a table-part
path, even the row's `Ссылка`, which the query resolver normalizes from the
physical owner-reference column to `StandardFieldId::Id`. Reuse the verified
document/catalog owner-reference rule for that identity. Prove the
section through the owner's complete native Config section collection,
indexed descriptors and an unambiguous DBNames VT mapping. A role must grant
owner Read, and an explicit section Read refusal excludes that role; duplicate
or unsupported section paths/restrictions remain undecidable. Other roles may
grant the same reference through the existing union rules.

Custom fields are resolved against the selected section's complete
`tabular_section_attributes` collection, with a unique loaded descriptor,
matching Config resource ownership and nonempty declared types. The owner's
flat `field_id(owner, name)` lookup is not a section-ownership test: the same
name may belong to a header or several sections. Known section fields and
the row number use owner Read with section/field overrides in the same role.
Overrides can be direct child GUIDs, owner-rooted member paths or an
attribute path rooted at its section; a descendant override does not deny
the section's reference. Duplicated or unsupported paths and child RLS
remain undecidable. UI rights and defaults do not grant owner Read.

The regression uses the reported invoice GUID and its native UNF Config
resource, with synthetic physical placement and fake rows identified as
such. Both the reference-only query and the complete query with
`Номенклатура` pass psql and the gateway protocol tests. Wildcard field
authorization also covers the row number. Metadata catalog checks assert
the visible item column. Negative checks cover child refusals in projection,
filters and sorting, neighboring sections and incomplete declarations.
PostgreSQL and MSSQL compilation retain owner-row RLS, while denied owner
Read stops execution before SQL. Negative field checks cover section Read
refusal, unknown/incomplete/duplicate membership, missing VT mapping and
unsupported fields. This does not claim a live production-database run.

## Gateway corpus acceptance

Replay the existing demo/UNF/BUH SDBL corpus through the authenticated gateway,
using its ordinary role/field checks, operator session bindings and a fake
database returning no rows. Render recorded scalar bindings as SDBL literals;
table-valued bindings and external temporary-table prerequisites remain
explicit limitations rather than invented server state. Every source entry
must have a result; refusals do not count as successful execution.

Pin per-entry SQLSTATE, result descriptions and generated SQL fingerprints.
A real psql process sends each entry as one Simple Query (-c preserves batch
boundaries) and must match the gateway outcome. This checks wire routing,
authorization and compilation, not database execution or result values.
Separate psql regressions assert rows/types, error recovery, bindings and
temporary-table lifetime. Baseline updates require explicit maintenance mode;
ordinary verification must not rewrite expected outcomes.
