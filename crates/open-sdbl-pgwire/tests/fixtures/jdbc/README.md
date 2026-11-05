# JDBC connection probe regression

`ConnectionProbe.java` runs the actual PostgreSQL JDBC driver supplied by
the caller. It checks login, `Connection.isValid`, literal keep-alive
queries, exact escaping/Unicode/aliases, prepared statements (threshold 1),
binary text and a read-only transaction followed by rollback. Both
`preferQueryMode=simple` and `extended` are exercised. The URL database
name is `EDIEvents`, matching the reported client URL; the fake gateway's
database journal must contain no probe SQL. Explicit transaction control
is checked separately through its begin/rollback events.
The runner also supplies the extracted DataGrip `ListSchemas` statement,
checking schema names, xid metadata, stable revisions and public first.
It also replays all nine statements from `datagrip-pre-schemas.sql` and
drains every result via JDBC type/name metadata and getObject. This forces
the driver's implicit TypeInfoCache JOIN USING/regproc lookups, including
the ACL arrays and xid values that a SQL-only replay does not cover.
The next thirteen statements in `datagrip-schema-objects.sql` cover event,
FDW, extension, language, cast and dependency metadata. The test then calls
DatabaseMetaData.getTables and asserts visible tables. pgjdbc 42.7.3 itself
projects NULL AS TABLE_CAT there; configured identity is checked separately
against current_database and pg_database. On current live unf both modes
pass all stages and getTables returns 5,228 tables/views. The gateway's
catalog limits were preserved; indexed equijoins replaced the Cartesian
intermediate that originally exceeded the native allocation budget.
It now first replays the current DataGrip bootstrap and native database
listing, checking configured database identity, TLS false on the plaintext
connection and stable numeric server-start epochs. The URL remains
EDIEvents while the configured backend is unf. Set
`OPEN_SDBL_JDBC_EXPECTED_DATABASE` for other backing database names in direct
live runs; it defaults to unf.

Run with Java 11+ and an existing driver jar (nothing is downloaded):

```sh
OPEN_SDBL_PGJDBC_JAR=/path/to/postgresql-42.7.3.jar \
  cargo test -p open-sdbl-pgwire pgjdbc_connection_and_keep_alive --offline \
  -- --ignored --nocapture
```

`OPEN_SDBL_JAVA` can select a Java launcher. Synthetic test credentials are
passed by the Rust runner through environment variables; the fixture
contains no live credentials. Running the source directly against a live
gateway requires `OPEN_SDBL_JDBC_USER` and `OPEN_SDBL_JDBC_PASSWORD` in the
environment, plus URL and query-mode arguments.
An optional third argument is the path to the schema-list SQL fixture.
For direct live runs `OPEN_SDBL_JDBC_SOCKET_TIMEOUT` overrides the default
10-second socket timeout. The Rust fake-engine runner still has its
30-second process deadline. Native catalog construction is timed separately
from the repeated query; no performance bound is claimed by this smoke test.

## Evidence and scope (2026-10-27)

- Installed pgjdbc 42.7.3 jar SHA-256:
  `a2644cbfba1baa145ff7e8c8ef582a6eed7a7ec4ca792f7f054122bdec756268`.
  REA artifact inventory
  `ev_155a490681e209bca1e1edaedd9bd2693d48e5cefc8cddc81c3e19421b68981d`
  confirms artifact identity only; static inventory does not prove runtime
  compatibility.
- Controlled replay before the fix reproduced `42601`, position 8, for
  `SELECT 'keep alive'`. After the fix the same actual driver passes both
  modes. Regular protocol tests also verify binary text and rejection in
  an aborted transaction until rollback.
- The exact IDE-configured keep-alive statement was not supplied. The
  regression covers single string-literal SELECT probes, including
  `SELECT 'Hello'` and `SELECT 'keep alive'`, without claiming an IDE trace.
- The rebuilt release gateway also passed the same actual-driver checks
  against the authorized current `unf` user in both modes, using
  `EDIEvents` as the URL database name on a temporary loopback listener.
  That listener was stopped; the existing gateway on 5433 was untouched.
- This is connection/probe coverage. Complete `DatabaseMetaData` and IDE
  navigator compatibility remain separate pending corpus tasks; the jar
  and a successful connection do not establish them.
- The DataGrip schema-list follow-up also passes its exact extracted SQL
  through pgjdbc 42.7.3 on both the fake engine and current live unf.
  The live run used a 120-second socket limit: cold construction was
  17.5–17.8 seconds per connection; repeated listings were at most 105 ms.
  The default 10-second test socket limit is insufficient for that native
  cold model build. This verifies schema-list SQL, not the complete IDE UI.
