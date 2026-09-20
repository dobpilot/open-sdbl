## 1. The mode

- [x] 1.1 `RestrictionMode` and `PrepareOptions`; `prepare_with_options`
  beside the existing `prepare`/`prepare_with`.
- [x] 1.2 `Prepared` stores the mode and passes it to every compilation;
  `CompileOptions` carries no mode, so it cannot lower one.
- [x] 1.3 `compile_statement` arms the catalog from the mode, not from the
  keyword; the AST and the source text are untouched.

## 2. Decisions

- [x] 2.1 `AccessDecision` with the three outcomes; `CompileOptions::decisions`.
- [x] 2.2 A missing decision, a duplicate decision and a broken condition
  are `Restriction` diagnostics naming object and tabular section.
- [x] 2.3 A denial renders the restricted wrapper with a false predicate.

## 3. Coverage

- [x] 3.1 Audit every place that turns a metadata object into a physical
  relation; route each through one guard.
- [x] 3.2 Reference joins — plain, composite and presentation — register
  their target and read through the restricted wrapper.
- [x] 3.3 Refuse hierarchy descents, filter criteria, the constants
  source, document journals, and a temporary table defined outside the
  restricted batch.
- [x] 3.4 `TempTable` records the mode it was compiled under.

## 4. The database layer

- [x] 4.1 `open-sdbl-db` answers with decisions; `Access::Denied` denies.
- [x] 4.2 Any `RestrictionError`, unread role, or absent user fails the
  whole answer.

## 5. Tests

- [x] 5.1 For PostgreSQL and MSSQL: a query without the keyword is
  protected; every statement of a mixed batch; nested query, union branch
  and explicit join; plain and composite dereference; a presentation read;
  a virtual table; a temporary table of the same batch; one target through
  several aliases.
- [x] 5.2 Explicit unrestricted decision; denial yields a false predicate;
  a missing decision fails; a failing expansion never compiles.
- [x] 5.3 A prepared restricted query cannot be compiled with less
  protection; `Statement` mode output is unchanged.
- [x] 5.4 Every refused construct has a test asserting the diagnostic.
- [x] 5.5 Assert the predicates in the generated SQL, not only the
  contents of the request.
- [x] 5.6 One live MSSQL test (`#[ignore]`) runs a denied target against a
  real base and asserts the server returns no row of it, so the SQL-text
  assertions are not mistaken for a result check.

## 6. Checks

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.3 `cargo test --workspace`
- [x] 6.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 6.5 `openspec validate --all --strict`
- [x] 6.6 README and `docs/query-language-support.md` state the mode, the
  covered reads and the refused constructs; archive the change.
