## Context

The compiler produces one `CompiledQuery` per call and has no notion of a
statement sequence. Nested sources already compile an inner statement in the
storage date domain and expose its columns as a `SourceScope` of synthesized
fields (`derived_source_scope`), which is exactly what reading a temporary
table needs. The databases are read-only copies: `CREATE TEMP TABLE` and
`#tables` are unavailable, so temporary tables are emulated with `WITH`
common table expressions, which PostgreSQL supports since 8.4 and SQL Server
since 2005, inside the generated PostgreSQL 9.0 / SQL Server 2008 envelope.

The platform documentation used for the rules: `G:INTO`,
`G:Work with temp table`, `G:WorkinWithBath`, `G:temp_ADD`, `G:INDEXBY`
(8.3.27.2342). `ДОБАВИТЬ` exists since 8.3.25; the help names no English
spelling, `ADD` is accepted as the natural counterpart.

Rulings taken with the maintainer: the manager lives in the core as a
public object; CTE names are `vt1..vtN`; `ДОБАВИТЬ` yields a new CTE via
`UNION ALL` with a strict structural check; `УНИЧТОЖИТЬ` generates nothing;
final `ПОМЕСТИТЬ` returns `Количество`; `ИНДЕКСИРОВАТЬ ПО` is validated and
ignored; ordering inside a definition needs `ПЕРВЫЕ`; diagnostics use a new
`TemporaryTable` kind; one change, version 0.3.3.

## Decisions

### Grammar

```text
batch      := statement (';' statement)* ';'*
statement  := query [index] | drop
query      := select [into] from … (union select …)* [order] [index]
into       := ('ПОМЕСТИТЬ' | 'ДОБАВИТЬ') identifier        -- after the first selection list
index      := 'ИНДЕКСИРОВАТЬ' 'ПО' ( fields | 'НАБОРАМ' '(' '(' fields ')' ['УНИКАЛЬНО'] (',' …)* ')' ) ['УНИКАЛЬНО']
drop       := 'УНИЧТОЖИТЬ' identifier
```

Empty statements between semicolons are skipped, so the existing
"repeated terminator" behaviour is preserved. `ПОМЕСТИТЬ`/`ДОБАВИТЬ` sit
after the selection list of the first branch, as in 1C; with a union the
whole union is stored. `ИНДЕКСИРОВАТЬ ПО` is accepted as the trailing
clause of a statement, after `УПОРЯДОЧИТЬ ПО`, and also directly before it,
because the Syntax Assistant lists clauses out of textual order. The five
new keywords are contextual identifiers in the parser (`ДОБАВИТЬ` is
documented to be readable as an alias), so fields named `Уникально` keep
working. `ПОМЕСТИТЬ` inside a nested query is a `Syntax` diagnostic.

AST:

```rust
struct BatchAst { statements: Vec<StatementAst> }
enum StatementAst { Query { query: QueryAst, into: Option<IntoAst>, index: Option<IndexAst> },
                    Drop { token, name } }
struct IntoAst { token, name, append: bool }
struct IndexAst { token, sets: Vec<Vec<FieldReference>> }   // UNIQUE flags parsed and dropped
```

`Parser::parse` returns a `BatchAst`; a batch of more than 64 statements is
a `WorkBudgetExceeded` diagnostic and every statement charges the work
budget like a nested statement.

### The manager

```rust
pub struct TempTablesManager { dialect: Option<SqlDialect>, fingerprint: Option<SnapshotFingerprint>,
                               next_id: u32, entries: Vec<Entry> }
struct Entry { name: String, cte: String /* vtN */, sql: String, columns: Vec<CompiledColumn>,
               dependencies: Vec<String> /* cte names */, visible: bool }
pub struct TempTable<'a> { name: &'a str, columns: &'a [CompiledColumn] }   // public view
```

Public surface: `new`, `Default`, `Clone`, `Debug`, `tables()` (visible
entries in definition order), `contains(name)`, `is_empty()`, `clear()`.
The manager stores compiled SQL rather than SDBL text: the definition's
parameters were inlined when it was compiled, so a later batch needs no
parameter values for tables it only reads, and re-emission is a string
copy. Entries record the referenced CTE names so that later statements can
pull their transitive closure. The first definition binds the manager to
the compiling dialect and snapshot fingerprint; a later use with another
dialect is a `TemporaryTable` diagnostic and with another snapshot a
`SnapshotMismatch`; `clear()` unbinds. A manager holds at most 256
definitions (visible or hidden), beyond which definitions fail with
`TemporaryTable`.

Entry points:

```rust
impl QueryCompiler<B> {
    fn compile_batch(&self, source, options: &CompileOptions, manager: &mut TempTablesManager)
        -> Result<Option<CompiledQuery>, QueryDiagnostic>;
    fn prepare_with(&self, source, manager: &TempTablesManager) -> Result<Prepared<B>, QueryDiagnostic>;
}
impl Prepared<B> {
    fn compile_batch(&self, snapshot, options: &CompileOptions, manager: &mut TempTablesManager)
        -> Result<Option<CompiledQuery>, QueryDiagnostic>;
}
```

Compilation works on a scratch copy of the manager state and commits it
only on success, so a failed batch leaves the manager untouched. `None`
means the batch ends with `УНИЧТОЖИТЬ` and there is nothing to execute.
`compile`, `compile_with`, `prepare`, `compile_with_presentations`, and
`Prepared::compile*` keep their signatures: they accept batches with a
throwaway manager, and a batch that returns no rows fails with a
`TemporaryTable` diagnostic because `CompiledQuery` always carries SQL.
`CompileOptions` stays `Copy` and unchanged; the manager is a separate
`&mut` argument precisely because it is mutated.

### Definitions as CTEs

`ПОМЕСТИТЬ Имя` compiles the statement with the nested-query rules
(`compile_query_ast` with `nested = Some(token)`: storage date domain,
no `*`, no deferred presentations, ordering only with `ПЕРВЫЕ`, inline
presentations allowed, other temporary tables readable) into the body of
CTE `vtN`, N taken from the manager counter, with the statement's output
labels and kinds as the table's columns. The name must not be visible
already (`TemporaryTable` "already exists"); a name hidden by `УНИЧТОЖИТЬ`
may be reused. Names are compared with `names_equal`.

`ДОБАВИТЬ Имя` requires a visible name (`TemporaryTable` otherwise) and
compiles the statement the same way, then defines
`vtM AS (SELECT <labels of vtK> FROM vtK UNION ALL <statement>)` with
`dependencies = [vtK] ∪ deps(statement)` and rebinds the name to `vtM`.
The structure check is positional and strict: equal column count, every
pair compatible by `ColumnKind::is_compatible_with`, reference columns
with identical target sets and the same `runtime_typed` width, `NULL`
compatible with anything. No widening happens, because the platform
raises a type error instead of widening when appended rows do not fit.
The resulting columns keep the labels and kinds of the first definition.

`УНИЧТОЖИТЬ Имя` needs a visible name (`TemporaryTable` otherwise), emits
no SQL, and marks the entry hidden. Its CTE stays in the manager because a
later definition may still depend on it.

`ИНДЕКСИРОВАТЬ ПО` is parsed for both forms and `УНИКАЛЬНО`; each field
must name an output label of the statement (alias or generated label),
else a `TemporaryTable` diagnostic at the field. Nothing is generated: CTEs
have no indexes and the platform documents the clause as a performance
hint only.

### Reading a temporary table

`ИЗ Имя [КАК Псевдоним]` (no dot after the identifier, which is what
separates it from `Вид.Имя` metadata sources) resolves the visible entry
and builds a `SourceScope` exactly like `derived_source_scope`, but from the
stored columns and rendered as `"vtN" AS "Псевдоним"`; without an alias
the table name is the alias, as in the platform. The same source form is
accepted in joins, nested queries, and `В (ВЫБРАТЬ …)` subqueries.
Dereference and presentation rules are those of derived sources: one-hop
dereference through a fixed single-target reference column, deferred
presentations allowed in the final statement, rejected inside definitions.
An unknown or hidden name is a `TemporaryTable` diagnostic at the token.

### The final statement

- Plain query: `WITH vtA AS (…), vtB AS (…) <statement>` where the WITH
  list is the transitive closure of the CTEs the statement references, in
  ascending id order (definition order guarantees every CTE precedes its
  users). A statement that reads no temporary table has no `WITH` prefix,
  so single-statement SQL is unchanged.
- `ПОМЕСТИТЬ`: `WITH … SELECT COUNT(*) AS "Количество" FROM "vtN"`.
- `ДОБАВИТЬ`: `WITH … SELECT COUNT(*) AS "Количество" FROM (<statement>) AS "vtM"`,
  counting only the appended rows, with the closure of the statement's
  dependencies.
- `УНИЧТОЖИТЬ`: `None` from `compile_batch`, diagnostic from `compile`.

The `Количество` column has the kind of `КОЛИЧЕСТВО(*)`. CTE identifiers
are quoted through the dialect like every other identifier, so `vt1` cannot
collide with `_Reference…` physical names.

Date handling follows nested sources: CTE bodies stay in the storage
domain and only the final projection applies the MSSQL year offset, so a
date read from a temporary table is corrected exactly once.

### Dialect notes

PostgreSQL before 12 materializes every CTE, which matches the platform's
temporary-table semantics; PostgreSQL 12+ inlines single-reference CTEs and
SQL Server always inlines, so a CTE referenced several times may be
evaluated several times. Results are identical either way; the
`MATERIALIZED` hint is not emitted because it is PostgreSQL 12+ only. SQL
Server requires `WITH` to start a statement; the generated text is a single
statement and the CLI sends it alone.

### Console

The console owns one `TempTablesManager` per session. Every statement goes
through `prepare_with(&manager)`, the presentation-plan cache, and
`Prepared::compile_batch(…, &mut manager)`. A `Some(query)` executes and
prints as today, so a `ПОМЕСТИТЬ` statement shows its `Количество` row; a
`None` prints the names that disappeared from the manager (compared before
and after) and executes nothing. `\tables` prints one line per visible
table: its name and columns as `label kind`; with no tables it says so.
`\refresh` clears the manager and prints a notice, because the stored SQL
belongs to the old snapshot. `ConsoleHelper` receives the visible names so
completion after `ИЗ`/`СОЕДИНЕНИЕ` offers them together with metadata
sources. Because the console still ends a statement at the first `;`, a
platform batch pasted whole executes as consecutive statements against the
same manager and yields the same final result.

### Alternatives considered

- Storing SDBL text in the console and re-prepending it: rejected by the
  maintainer in favour of a core manager, and it would re-bind parameters
  on every query.
- A batch terminator other than `;` for the console: breaks the habit of
  pasting 1C text.
- Returning a `Vec<CompiledQuery>` per batch (`ВыполнитьПакет`): the Trino
  connector and the console execute one statement; the platform's
  `Выполнить()` semantics (last result) cover the use case.

## Risks

- A definition compiled with parameters is frozen; changing `\set` values
  later does not refresh it. `\tables` and the docs say so.
- Deep `ДОБАВИТЬ` chains produce nested `UNION ALL` CTEs; the 256-entry
  bound and the work budget cap the size.
