## Why

1C queries written for real configurations start with `ВЫБРАТЬ РАЗРЕШЕННЫЕ`
whenever the caller may lack rights to some rows, and the platform then
applies the role restrictions of the current user to every table the
statement reads. The compiler does not know the keyword at all (`РАЗРЕШЕННЫЕ`
lexes as an identifier and fails with `Syntax`), so those queries cannot run
in the console, and an application embedding the library has no way to
enforce row-level security on the SQL it generates.

## What Changes

- The lexer SHALL recognize `РАЗРЕШЕННЫЕ`/`ALLOWED`; the parser SHALL accept
  it only in the first top-level branch of a statement, before
  `РАЗЛИЧНЫЕ`/`ПЕРВЫЕ`, and SHALL apply it to the nested queries and union
  branches of that statement. Statements of a batch are independent.
- Preparation SHALL collect a `RestrictionRequest`: the metadata objects
  (and tabular sections) that statements with the keyword read, so that the
  application can supply an `AccessRestriction` per target. This is the
  second application callback of the library, shaped like the presentation
  request: no closures, no I/O.
- A restriction body is SDBL condition text in the style of 1C
  role-restriction templates: fields of the target unqualified,
  dereferences, `В (ВЫБРАТЬ …)` and `&Параметр` allowed. Raw SQL is
  impossible. Restrictions see only session parameters.
- `SessionParameters` is a new value shared by every query and every
  restriction of a session: `&Имя` resolves query parameters first, then
  session parameters; an unused session parameter is not an error.
- A restricted plain source renders as a derived table
  `(SELECT <columns> FROM <table> AS "__restricted" [LEFT JOIN …] WHERE
  <condition>)`, for every join kind; slices, balances, and turnovers
  conjoin the condition into their virtual-table predicate and therefore
  accept direct fields only, as the virtual-table condition does.
- A target without a restriction leaves the SQL byte-identical; nothing is
  merged algebraically with the query's own `ГДЕ`.
- New `QueryDiagnosticKind::Restriction`, positioned inside the restriction
  text and naming the target; a restriction whose target no statement reads
  with the keyword, or supplied twice, is an error.
- Dereferenced tables (`Т.Контрагент.Наименование`) are not restricted by
  this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: `РАЗРЕШЕННЫЕ`/`ALLOWED` keyword.
- `query-compilation`: restriction request and restrictions in
  `CompileOptions`, session parameters, restricted source rendering,
  `Restriction` diagnostics.

## Impact

- `src/lexer.rs` (keyword table), `src/query/core/parser.rs`, `ast.rs`
  (`allowed` on statements), `params.rs` (`SessionParameters`,
  `AccessRestriction`, `RestrictionTarget`, `RestrictionRequest`,
  `CompileOptions` builder methods), `diag.rs`, `resolve.rs`
  (`CompilationCatalog` restriction state), `codegen/sources.rs`,
  `virtual_tables.rs`, `select.rs`, `batch.rs`, `entry.rs`, `src/query.rs`
  (`Prepared::restriction_request`).
- CLI keeps compiling unchanged: it passes no restrictions and no session
  parameters. Console commands are a separate change
  (`restrict-cli-command`).
- README and `docs/query-language-support.md`.
