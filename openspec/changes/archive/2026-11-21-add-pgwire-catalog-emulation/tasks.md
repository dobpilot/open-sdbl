# Tasks

## 0. Gate

- [x] 0.1 `openspec validate add-pgwire-catalog-emulation --strict` passes before any code.

## 1. Client corpus

- [ ] 1.1 Record the introspection corpus against `postgres:13` with `log_statement = all`: psql 13 and psql 18 (`\dn`, `\dt`, `\dt pattern`, `\dv`, `\d`, `\d+`, `\dT`), pgjdbc 42.7.x (`getSchemas`, `getTables`, `getColumns`, `getPrimaryKeys`, `getTypeInfo`, prepared statement), psycopg 3 `TypeInfo.fetch`, psqlODBC `SQLTables`/`SQLColumns`; commit deduplicated statements under `crates/open-sdbl-pgwire/tests/fixtures/catalog-corpus/` with a README naming client versions and the recording method. Verified by the README and by every statement carrying its client tag.
- [ ] 1.2 Add DBeaver's navigator statements to the corpus from a session the maintainer records the same way (connect, expand schemas, open a table's columns). Verified by a `dbeaver` group in the corpus; pause for the maintainer if no recording is available.
- [ ] 1.3 Derive from the corpus the exact list of catalog relations, columns and functions required, and record it in the corpus README. Verified by the README list matching the replay test's coverage report.

## 2. Catalog model

- [x] 2.1 Build relations and kind schemas from the snapshot via `object_query_name`, tabular sections and supported register virtual tables. Verified by unit tests over the existing catalog, document and register fixtures listing expected schema/relation names.
- [x] 2.2 Derive columns by compiling `ВЫБРАТЬ * ИЗ <relation>` in Restricted mode, mapping kinds with the gateway's result type mapping. Verified by a test asserting catalog type OIDs equal `RowDescription` OIDs for the same fields.
- [x] 2.3 Apply visibility: drop relations with denied or undecidable decisions and columns whose single-field check fails. Verified by tests for a denied object, a restricted object (listed) and a denied field (hidden).
- [x] 2.4 Synonyms as `pg_description` rows. Verified by a test reading an object and a field synonym.
- [x] 2.5 Stable OIDs over the whole snapshot. Verified by a test comparing OIDs across two users with different rights and across two builds.
- [x] 2.6 Cache the model per connection keyed by the gateway state; rebuild after refresh. Verified by a test replacing `StateHandle` state and observing new relations.
- [x] 2.7 Model build bound on a synthetic 10,000-relation snapshot. Verified by a timing test with the documented bound.

## 3. SQL subset

- [x] 3.1 PostgreSQL lexer subset (quoted identifiers, `E''`, dollar quoting, `$n`, `::`, comments). Verified by unit tests including malformed input.
- [ ] 3.2 Parser for the SELECT subset in the design with character positions. Verified by round-trip/AST tests and `42601` position tests.
- [x] 3.3 Name resolution, typing, casts (`regclass` & co.) and `$n` type inference; Describe without evaluation. Verified by unit tests and an Extended Query Describe test.
- [x] 3.4 Evaluator with joins, subqueries (correlated), `UNION`, aggregates, `ORDER BY`/`LIMIT`, and budgets (`54001`). Verified by unit tests and the cartesian-explosion scenario.
- [ ] 3.5 Functions and operators required by the corpus; unknown function `42883`, unsupported construct `0A000`. Verified by per-function tests and the unknown-function scenario.
- [x] 3.6 Regex subset for `~`, `~*`, `!~`, `!~*`, cross-checked against cases evaluated on the recording server. Verified by a table-driven test.
- [x] 3.7 Text and binary result encoding for catalog types. Verified by tokio-postgres decoding catalog rows in binary.
- [x] 3.8 Fuzz target `pgwire_catalog_sql` in `fuzz/`. Verified by `cargo check` of the fuzz workspace.

## 4. Gateway integration

- [x] 4.1 Route catalog-intended text (Simple and Extended Query) to the catalog; keep SDBL routing for everything else; mixed relations go to SDBL. Verified by tests for the three routing scenarios of the `pgwire-catalog` spec.
- [x] 4.2 `SHOW search_path` and `pg_settings` values; catalog statements inside transaction blocks and with a lost database session. Verified by protocol tests.
- [x] 4.3 Corpus replay test: every corpus statement evaluates without error against the fixture snapshot; selected statements assert results (`\d` columns and types). Verified by the test and its coverage report.
- [ ] 4.4 End-to-end tokio-postgres tests over the fake engine covering every `pgwire-catalog` scenario, with the fake database journal empty. Verified by the tests.
- [x] 4.5 Ignored live tests running psql `\dn`, `\dt`, `\d` against `open-sdbl serve`. Verified by `cargo test -p open-sdbl-cli --test serve_live -- --ignored` documentation in README.

## 5. Documentation and finish

- [x] 5.1 README (Russian): what the catalog shows, the mapping, visibility, supported clients, limits and the non-goals (no SQL reading of 1C tables). Verified by review of the section.
- [x] 5.2 Bump all workspace packages to 0.11.0. Verified by `cargo metadata`.
- [x] 5.3 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` all pass.
- [x] 5.4 `openspec validate add-pgwire-catalog-emulation --strict` passes after implementation.

## Workflow follow-up

- `openspec archive add-pgwire-catalog-emulation` after review.

## Verification notes (2026-10-26)

- Maintainer instruction: verify with psql. The full change stays active;
  non-psql recordings and compatibility are not claimed. Tasks 1.1–1.3,
  3.2 (the full planned grammar), 3.5 (the complete multi-client registry),
  and 4.4 (every client scenario) remain open.
- Current corpus: 21 psql 13 statements and 24 psql 18 statements, replayed
  over a model with matching describe OIDs and asserted columns/types/comments.
- Both real psql 13.23 and 18.6 passed `psql_catalog_introspection`, including
  `\dn`, `\dt`, `\dv`, `\d`, `\d+`, patterns and `\dT`. These runs use an
  authenticated fake engine and assert an empty database journal. Real 1C
  base catalog tests were added but not run without fixture credentials.
- Protocol tests cover binary OIDs/arrays, parameter description, denied and
  restricted relations, hidden fields, state refresh, lost database sessions,
  transaction blocks, and recovery after a `54001` allocation/row budget error.
- Release model build on 10,000 synthetic relations: 251 ms isolated,
  509 ms during concurrent checks; bound <1 second. The measurement includes
  candidate identities, restricted preparation/compilation and catalog rows.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`
  and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --offline`
  pass. Final gateway suite: 80 passed, 2 ignored; gateway rustdoc passes.
- Fuzz target checks and a 2,000-input smoke run pass. The smoke binary was
  built with ordinary Cargo, without coverage/sanitizer instrumentation;
  a sustained coverage-guided campaign remains separate work.
- Targeted strict OpenSpec validation passes. Repository-wide strict validation
  reports 36 passed / 14 failed: other existing specs/changes exceed the
  requirement-length warning threshold. Those contracts were not rewritten.
- `cargo test --workspace --offline` passes, including the existing
  1,726-query SDBL corpus and database metadata/cache suites. Formatting
  and `git diff --check` pass. No commit or archive was made.

## 6. Fix live startup-snapshot reuse

- [x] 6.1 Build and reuse field and separator lookup indexes during snapshot finalization; preserve duplicate/extension semantics and refresh isolation.
- [x] 6.2 Retain relation inventory at gateway startup and use indexed field comments while keeping all per-user rights checks.
- [x] 6.3 Add a representative custom-attribute-heavy regression, covering first and repeated introspection and no database I/O; replace the ID-only performance claim with measured evidence.
- [x] 6.4 Run formatting, Clippy, relevant tests, workspace tests, rustdoc and strict change validation; build the release CLI.

Populated regression evidence: `populated_catalog_first_and_repeated_request`
(release) measured 283 ms cold / 6 ms warm for 1,000 relations and 20,000
custom attributes, including complete permissions and a second connection;
its fake database journal is empty. `query_snapshot_indexes` verifies field
names/origins, repeated preparation, source-order duplicate descriptors and
old/new snapshot isolation. The previous ID-only figure does not establish
performance for a populated configuration. Release CLI rebuilt for restart.

The fix also passes real psql 18 `\dt`, `\d` and repeated `\dt` on the
populated snapshot (1.73 s for the complete script, empty DB journal).
Workspace tests, 34 targeted access checks, warning-denied Clippy and
rustdoc, formatting and strict change validation pass. The release CLI is
rebuilt; existing running processes require a restart to use the fix.
Repository-wide OpenSpec validation retains the same 14 unrelated long-
requirement failures (36 items pass).

## 7. Review fixes

- [x] 7.1 Charge concatenated string allocations before construction, including intermediate scalar results; test budget failure and connection recovery.
- [x] 7.2 Preserve declared and inferred parameter types throughout evaluation and casts; test binary protocol results, NULLs and nested queries.
- [x] 7.3 Apply UNION ordering, LIMIT and OFFSET to the complete set operation; test multiple branches and nested unions.
- [x] 7.4 Run formatting, warnings-denied Clippy and rustdoc, workspace tests, psql verification and strict change validation; rebuild release CLI.

Review-fix validation: the full workspace test run passes, as do formatting,
warnings-denied all-feature Clippy and rustdoc, and strict validation of this
change. The gateway all-feature suite passes 86 tests (4 ignored), plus its
rustdoc test. Real psql 18.6 introspection and UNION verification pass. The
release CLI was rebuilt and its help command exits successfully. Global
strict OpenSpec validation retains the same 36 passed / 14 unrelated failures.

## 8. Tabular-section owner reference regression

- [x] 8.1 Replace the blanket section refusal for a verified owner reference with the existing owner-reference mapping; preserve role checks, explicit section refusals and RLS.
- [x] 8.2 Add positive/negative field-access and gateway regressions for the reported incoming-invoice query using its loaded native Config descriptors.
- [x] 8.3 Run relevant tests, required workspace checks, strict change validation and a release CLI build.

Tabular-reference verification: 35 targeted access tests and the full workspace
test run pass, including the reported query over PostgreSQL and MSSQL dialects
with owner RLS and a denied-owner check before SQL. Real psql 18.6 passes the
exact reported query on a fake engine with the native invoice Config resource
and synthetic physical placement. Formatting, warnings-denied all-feature
Clippy/rustdoc and strict change validation pass. The release CLI is rebuilt
and its help command succeeds. Global OpenSpec validation retains the same
36 passed / 14 unrelated long-requirement failures.

## 9. Complete tabular-section field authorization

- [x] 9.1 Resolve section custom fields using native section membership; authorize verified fields and row number while preserving explicit child refusals and role boundaries.
- [x] 9.2 Extend the native invoice fixture with item fields; test the full reported query, wildcard reads, foreign/incomplete membership, child refusals and RLS through both dialects and psql.
- [x] 9.3 Run checks and build the release CLI.

Section-field verification: the complete reported query with `Номенклатура`
passes real psql 18.6 against the native-invoice test fixture, with fake DB
rows and synthetic placement. The final access suite passes 36 tests;
all-feature gateway tests pass 87 tests (5 ignored) plus rustdoc. Tests of
the affected DB, CLI and pgwire crates pass, followed by final targeted access
and gateway/psql checks for section-rooted child entries. Wildcard and row
number authorization, foreign-section and incomplete/duplicate declarations,
projection/filter/sort refusals, role union and owner RLS are covered.
Formatting, all-feature warnings-denied workspace Clippy/rustdoc, strict
change validation and the rebuilt release CLI help check pass. Global strict
OpenSpec validation retains the same 36 passed / 14 unrelated failures.

## 10. SDBL corpus through the gateway and psql

- [x] 10.1 Share the existing corpus reader/bindings with gateway tests and replay all 1,726 entries over the frontend protocol without bypassing authorization.
- [x] 10.2 Record per-entry gateway outcomes, separate successful executions and known refusals, and compare real psql replay against those outcomes.
- [x] 10.3 Add psql regressions for errors/recovery, scalar bindings, tabular fields, types and connection-local temporary tables; bound subprocess lifetime.
- [x] 10.4 Run replays, relevant checks and strict change validation, and document reproduction commands and limits.

Corpus verification: all 1,726 entries were recorded and then replayed without
baseline updates through raw pgwire (100 s) and real psql 18.6 (122 s).
Both paths agree on every SQLSTATE and generated-SQL fingerprint: 641 OK,
1,085 refusals. Refusals are preserved with their exact messages and never
counted as supported queries. The independent compiler golden corpus still
passes after sharing its reader and bindings (108 s). Four new focused psql
tests pass, as do the existing catalog, populated-catalog and full invoice
psql tests through the bounded shared runner. The ordinary gateway suite and
the new fast baseline-completeness test pass. Formatting, warnings-denied
workspace all-feature Clippy/rustdoc, diff checks and strict change validation
pass. Global strict validation retains the same 36 passed / 14 unrelated
long-requirement failures. Reproduction and input-adaptation/data limitations
are documented in `tests/fixtures/sdbl-corpus/README.md` of the gateway crate.

## 11. JDBC connection probe regression (2026-10-27)

- [x] 11.1 Reproduce SQLSTATE 42601 at position 8 using the installed pgjdbc 42.7.3 and a string-literal probe.
- [x] 11.2 Add a bounded local probe decoder, protocol/escaping/routing regressions and an opt-in real JDBC test; preserve the SDBL path and avoid catalog construction/database I/O.
- [x] 11.3 Verify real JDBC modes and the current live base; run checks, validate this change and rebuild the release CLI. Document the checked scope without claiming full JDBC introspection support.

The installed pgjdbc 42.7.3 passes login/isValid, string probes, exact text,
aliases, prepared results and rollback in Simple and Extended Query, both
against the fake engine and the current authorized unf user on a temporary
release gateway with URL database name EDIEvents. The existing 5433 server
was not restarted. Full JDBC introspection and IDE navigator recordings
remain pending. All 1,726 SDBL cases retain their recorded SQLSTATE and
generated-SQL fingerprints (100 s). The affected CLI/pgwire tests pass
(163 passed, 17 ignored), along with warning-denied workspace Clippy and
rustdoc, formatting/diff checks and strict validation of this change.
Release CLI rebuilt; a running gateway requires restart to use the fix.

## 12. DataGrip schema enumeration (2026-10-27)

- [x] 12.1 Preserve the installed DataGrip ListSchemas SQL as a fixture with provenance and add a failing regression.
- [x] 12.2 Expose schema xmin as a state-scoped virtual xid with correct metadata/encoding and wildcard behavior; retain visibility and refresh isolation.
- [x] 12.3 Verify the exact query through Simple/Extended Query, actual pgjdbc and a temporary live gateway; run relevant checks and rebuild release.

The exact extracted ListSchemas query passes with the expected five column
types through text/binary protocol and real pgjdbc 42.7.3 in both modes.
Refresh tests preserve old portal rows/revisions and hide newly denied
schemas. Live unf returns eight schemas through JDBC and psql on a temporary
release gateway; the existing 5433 process was untouched. Native cold model
construction took 17.5–17.8 seconds per connection (requiring a longer live
test socket limit than the default 10 seconds); repeated listings were at
most 105 ms. No complete IDE UI trace or full-introspection compatibility is
claimed. The affected CLI/pgwire suite passes 167 tests (17 ignored), the
all-feature gateway suite passes 96 tests (13 ignored), and real psql catalog
introspection passes. Workspace warning-denied Clippy/rustdoc, formatting,
diff checks, strict change validation and the release build pass.

## 13. DataGrip bootstrap and configured database identity

- [x] 13.1 Record the current failed bootstrap statements and native ListDatabases query, and reproduce failures.
- [x] 13.2 Bind catalog identity to the configured backend; implement transport/start-time probes and shared descriptions with truthful types/values and no SQL forwarding.
- [x] 13.3 Verify the bootstrap/database/schema sequence through actual JDBC, protocol tests and a temporary live gateway; run checks and rebuild release.

The current DataGrip log showed that SSL status and rounded startup-epoch
probes failed before ListSchemas. Both now answer from gateway runtime
facts, including correct binary boolean/numeric results. Tests verify TLS
true/plaintext false, startup-time stability across clients/refresh and
configured database identity in pg_database/current_database/current_catalog
and information_schema. Client URL/default database names no longer rename
the configured base. Actual pgjdbc 42.7.3 passes the recorded bootstrap and
database/schema listings in both modes on the fake engine and current live
unf; live psql also passes with its database defaulting to the login name.
The affected suite passes 169 tests (17 ignored); warnings-denied workspace
Clippy/rustdoc, formatting/diff checks and strict change validation pass.
Release rebuilt; existing 5433 process untouched and requires restart.

## 14. DataGrip catalog stubs and metadata fetch

- [x] 14.1 Record the full current pre-schema session, including metadata getters that trigger pgjdbc lookups.
- [x] 14.2 Add typed stubs/system fields/functions, UNION DISTINCT and validated INNER/LEFT JOIN USING semantics.
- [x] 14.3 Replay every recorded query and metadata/object getter through real JDBC and psql; verify live unf, checks and release build.

Nine statements from the 15:52 session are retained without connection/log
data. Typed empty stubs, rolbypassrls=false and physical-catalog xmin make
them describable/evaluable. UNION DISTINCT and JOIN USING are tested for
duplicate elimination, NULLs, left-join retention, merged/qualified/star
columns, missing/ambiguous/duplicate keys and invalid ON plus USING.
Actual pgjdbc 42.7.3 metadata getters/getObject exposed a further regproc
array_in cast; the modeled symbol and array typinput now support that lookup.
Both JDBC query modes pass all nine statements and metadata/object fetching
on the fake engine and current live unf. Native psql replay and its separate
fake-engine sequence regression pass. The affected suite passes 172 tests;
warning-denied workspace Clippy/rustdoc, formatting/diff checks and strict
change validation pass. Release rebuilt; existing 5433 process untouched.

## 15. DataGrip schema-object introspection stubs

- [x] 15.1 Record the new schema-object session and reproduce all listed failures, including SQL-routing errors.
- [x] 15.2 Add typed relations/regclass identities, virtual recovery/transaction probes and an empty extension-version table function with array_agg.
- [x] 15.3 Exercise the recorded queries and actual table listing through JDBC/psql, verify live unf, run checks and rebuild release.

Thirteen additional schema-object statements are retained with typed empty
catalogs, regclass identities, virtual recovery/transaction values and an
empty extension-version table function. array_agg tests cover types, NULLs,
empty input, invalid arguments/nesting and allocation limits. Actual JDBC
getTables exposed a native allocation-budget failure after SQL fixes; indexed
ON/USING and necessary inner-WHERE equality keys remove the Cartesian
intermediate without increasing limits. Index work/memory is charged;
OR/LEFT/NULL and mixed runtime-value controls preserve fallback semantics.
The populated regression asserts 1,000 tables, >20,000 comments, correct
descriptions and no database SQL. Live unf passes all 25 recorded statements
through psql and JDBC metadata/object getters; JDBC getTables returns 5,228
tables/views in both modes. The affected suite passes 176 tests; workspace
warning-denied Clippy/rustdoc, formatting/diff checks and strict change
validation pass. Release rebuilt; existing 5433 process untouched.

## 16. DataGrip full schema introspection

- [x] 16.1 Capture and reproduce every distinct query from the full schema session.
- [x] 16.2 Implement missing typed stubs, scalar functions, ordered aggregates and natural joins; retain evaluation budgets.
- [x] 16.3 Replay the corpus with result metadata in both JDBC modes, verify populated/live results, run checks and rebuild release.

All 30 distinct full-schema statements from the 20:57 session are retained
with JDBC placeholders. Typed sequence/operator/aggregate/opclass/opfamily/
amop/amproc inventories and missing procedure/constraint fields describe
correctly; translate, greatest, ordered aggregates, a single projected
unnest and NATURAL INNER/LEFT joins are covered, including invalid/empty
inputs. Indexed OR alternatives preserve duplicate/NULL/LEFT behavior.
Early filters, projected intermediates and empty-inner inventories remove
allocation failures without increasing the 64 MiB/50-million-work limits.
The populated protocol regression returns 1000 table/comment targets and
21000 attribute ACL rows with no database SQL. Actual pgjdbc 42.7.3 passes
240 queries across all eight live unf schemas in each of Simple/Extended
mode, fetching result type names and objects. Native DataGrip discovery
returns 5228 user tables/views plus 48 system relations; JDBC getTables
also returns 5228. Real psql passes both recorded-sequence regressions.
The affected CLI/pgwire suite passes 181 tests; warning-denied workspace
Clippy/rustdoc, formatting/diff checks and strict change validation pass.
Final release rebuilt; temporary test gateways stopped and the existing
5433 process was not restarted. This covers the recorded full-schema stage,
not unrecorded later IDE stages or other client versions.

## 17. Russian reference labels and omitted metadata kinds

- [x] 17.1 Reproduce ID labels and missing task/accounting/process/exchange objects against the loaded snapshot.
- [x] 17.2 Use Ссылка for standard wildcard references; retain readable relations independently of proven column mappings, without weakening access checks.
- [x] 17.3 Cover all four kinds, denied objects/fields, aliases and sections; verify live listings and run checks/rebuild.

The live unf snapshot contains all requested owner kinds (one task, one
accounting register, nine business processes, 27 exchange plans). They were
omitted because all projected fields failed unsupported native field-right
mappings. Catalog construction now retains an owner/view after proven object
Read, independently of column mapping or row-compilation context; empty
sections still require the existing child-right proof. No field permissions
were broadened. Tests cover all four kinds, denied owners and sections,
missing compilation context, Russian reference labels, explicit ID aliases,
custom ID fields and tabular-section owner references. The 47 corpus changes
(29 demo, ten unf, eight buh) were checked to change only AS ID to AS Ссылка;
source queries and physical SQL expressions are unchanged.
Final live psql lists the four schemas (nine process relations, one task,
27 plans, one accounting table plus five views), confirms Ссылка for
Номенклатура and returns 6807 user tables/views. All fields of the four
returned kinds remain unproven and absent in this base; this change does
not claim their data access is implemented. Recorded JDBC full-schema
queries pass in Extended mode against live unf and both modes on the fake
engine; real psql regressions pass. The final section-boundary regression
and complete gateway suite pass, and release is rebuilt. Temporary gateways
were stopped without changing the existing 5433 listener.

Workspace all-feature tests pass: 1369 passed, 32 ignored, zero failures.
The final gateway rerun after the section-boundary guard passes 114 tests
plus its rustdoc test. Workspace warning-denied Clippy/rustdoc, formatting,
diff checks and strict validation of this change pass. No commit or archive
was created.
