# Emulate the PostgreSQL system catalog in the pgwire gateway

## Why
The gateway answers SDBL, but GUI and driver clients learn what a database
holds by querying `pg_catalog` and `information_schema` with real SQL.
Today those queries are compiled as SDBL and fail, so DBeaver shows an
empty tree, `psql \dt` and `\d` report errors, and pgjdbc
`DatabaseMetaData.getTables`/`getColumns`, psycopg and psqlODBC cannot
introspect the base. Answering them from the 1C metadata the gateway
already holds makes the objects of a 1C base browsable, with their
attributes, types and synonyms, without a single extra database query.

## What Changes
- A virtual, read-only system catalog in `open-sdbl-pgwire`, built per
  connection from the metadata snapshot and the authenticated user's
  rights: `pg_catalog` relations (`pg_namespace`, `pg_class`,
  `pg_attribute`, `pg_type`, `pg_description`, `pg_database`, `pg_roles`,
  `pg_settings`, `pg_index`, `pg_constraint`, `pg_proc` and the others the
  supported clients read) and the `information_schema` views `schemata`,
  `tables`, `columns`, `views`, `table_constraints`,
  `key_column_usage`.
- Mapping: one schema per 1C object kind with its SDBL name
  (`Справочник`, `Документ`, `РегистрНакопления`, …); one table per
  object (`Номенклатура`), tabular sections as `Объект.ТабличнаяЧасть`,
  register virtual tables as views (`ЗамерОстатков.Остатки`); columns are
  the queryable fields with the PostgreSQL types the gateway already
  reports for results; synonyms become object and column comments.
- Only what the user may read is listed: an object whose read decision is
  denied, and a field the complete field-rights check does not prove
  readable, are absent from every catalog relation.
- A bounded SQL subset evaluated in memory over those virtual relations:
  `SELECT` with joins, subqueries, `UNION`, `CASE`, casts, `ORDER BY`,
  `LIMIT`/`OFFSET`, aggregates, PostgreSQL operators (including the
  regular-expression operators psql uses) and the catalog functions the
  supported clients call, in Simple and Extended Query with `$n`
  parameters. Unsupported constructs fail with `0A000`/`42883`; nothing
  is ever sent to the database.
- Routing: a text is evaluated by the catalog engine only when it parses
  as that SQL subset and every relation it reads is a virtual catalog
  relation (or it reads none and calls only catalog functions); every
  other text keeps being compiled as SDBL, so the existing SDBL
  diagnostics and the client-chatter whitelist are unchanged.
- Supported clients for the first version: psql 13–18 (`\dn`, `\dt`,
  `\dv`, `\d`, `\d+`, `\dT` subset), DBeaver and pgjdbc 42.x
  `DatabaseMetaData`, psycopg 3 type introspection, psqlODBC catalog
  functions — verified against a recorded corpus of their real
  introspection queries.

## Capabilities

### New Capabilities
- `pgwire-catalog`: the virtual PostgreSQL system catalog the gateway
  exposes — what it lists for a user, how 1C objects map to schemas,
  relations, columns and types, the SQL subset it evaluates, and how
  unsupported catalog SQL fails.

### Modified Capabilities
- `pgwire-gateway`: "Serve the PostgreSQL protocol for SDBL" — query texts
  are SDBL except client chatter and catalog SQL routed to the virtual
  catalog; "Never forward client text to the database" gains a scenario
  for catalog SQL.

## Impact
- Code: new modules in `crates/open-sdbl-pgwire` (catalog model, SQL
  subset lexer/parser/evaluator, regex subset), routing in the connection
  state machine; possibly small read-only accessors in `open-sdbl` for
  object synonyms and queryable fields of virtual tables.
- No new third-party production dependency; no change to `open-sdbl-db`
  behaviour; no SQL ever reaches the DBMS from catalog queries.
- Tests: a fixture corpus of introspection queries recorded from the
  supported clients against a scratch PostgreSQL 13+ server, replayed
  against the virtual catalog; end-to-end `tokio-postgres` tests.
- Version bump of all workspace packages to 0.11.0.

## Non-goals
- Translating SQL that reads 1C tables (`SELECT * FROM
  "Справочник"."Номенклатура"`, DBeaver's data tab) into SDBL — a
  separate change; such text keeps failing as SDBL.
- Writable catalog, DDL, `pg_stat_*` statistics with real numbers,
  indexes and physical sizes of the 1C tables, `pg_hba`/roles management.
- Full PostgreSQL SQL compatibility beyond what the recorded client
  corpus needs.
