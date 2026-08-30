## Context

`open-sdbl` currently tokenizes SDBL and resolves logical metadata names to
PostgreSQL tables and columns. `open-sdbl-cli` can load a `MetadataSnapshot`
through `tokio-postgres`, but closes the connection after printing it. The REPL
needs a reusable connection session, deterministic compilation in the core,
and dynamic row rendering in the CLI.

## Goals / Non-Goals

**Goals:**

- Compile useful read-only single-source 1C queries without guessing physical
  names.
- Keep all syntax and mapping decisions testable without a database.
- Keep the REPL alive after syntax, lookup, or PostgreSQL query errors.
- Make discovery output logical-name-first while showing physical evidence.

**Non-Goals:**

- Implement the entire 1C query language in this change.
- Execute 1C queries through the 1C server runtime.
- Support mutations, joins, unions, grouping, temporary tables, parameters, or
  automatic decoding of reference byte order into typed 1C values.
- Add line-editing/history dependencies.

## Decisions

### Compile an explicit SDBL subset in the core

The compiler accepts one `SELECT`/`ВЫБРАТЬ` with optional `DISTINCT`, `TOP`/
`ПЕРВЫЕ`, a projection list or `*`, one `FROM`/`ИЗ` metadata source, an
optional alias, a basic `WHERE`/`ГДЕ` expression, and `ORDER BY`/
`УПОРЯДОЧИТЬ ПО` with `ASC`/`ВОЗР` and `DESC`/`УБЫВ`. Expressions allow known fields, string/number/boolean/NULL
literals, parentheses, comparison/arithmetic operators, and AND/OR/NOT.
Anything outside that grammar is rejected with a source position.

Metadata sources use `<kind>.<name>` with English or Russian kind names.
Fields use Config descriptor names or documented standard bilingual names such
as `Код`/`Code`, `Наименование`/`Description`, and `Ссылка`/`ID`. Physical table
and column identifiers always come from the resolved snapshot and are quoted.

### Cast projection values to text

1C PostgreSQL schemas contain platform domains and binary reference columns.
Generated projections cast every physical member to PostgreSQL `text`, giving
the CLI one nullable representation type without teaching the application
about every platform domain. Compound logical fields expand to separately
labeled physical members. Predicates still operate on native columns and typed
literals where the bounded grammar permits them.

### Use one connection and one transaction per statement

The REPL loads metadata once at startup. Every entered query starts a new
read-only `READ COMMITTED` transaction, verifies its mode, executes one
generated SELECT, and commits or rolls back. This avoids a long-lived snapshot
and ensures a failed statement cannot poison the session.

### Keep meta commands local to the snapshot

`\dt`, `\di`, and `\d` do not issue heuristic catalog scans. They render the
already resolved Config/DBNames/SchemaStorage/catalog snapshot. `\d` accepts a
qualified logical name, a unique bare logical name, or an exact canonical
physical table name; ambiguous or absent names produce a diagnostic.

### Use a minimal asynchronous line loop

The CLI uses Tokio standard-input support and explicit prompts. A semicolon
terminates a multiline query; backslash commands are single-line. EOF and
`\q` exit successfully. A line-editing dependency was rejected for the first
version to keep terminal behavior portable and scriptable.

## Risks / Trade-offs

- **[Users assume full 1C compatibility]** → Help and diagnostics name the
  bounded subset and reject unsupported clauses before execution.
- **[Metadata changes during a session]** → `\refresh` reloads the snapshot on
  demand from a fresh read-only transaction.
- **[Wide binary/compound output]** → Cast to text, escape controls, and expand
  members with stable labels rather than silently dropping representation data.
- **[Dynamic output is large]** → Stream rows returned by the bounded query and
  show a final row count; pagination is deferred.

## Migration Plan

Add compiler APIs and tests first, then refactor the CLI connection lifecycle
and add the REPL. Existing `lex` and `metadata postgres` commands keep their
behavior. No database migration or mutation is required.
