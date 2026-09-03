# Design

## Guiding constraints

- The core crate stays dependency-free and I/O-free; every fix below is
  deterministic library logic.
- `Display` messages of existing diagnostics stay textually stable wherever
  tests do not prove them wrong; machine-readable kinds are added alongside,
  not instead of, the message.
- The work is ordered so each phase leaves the workspace green
  (`fmt`, `clippy -D warnings`, tests, rustdoc): reliability fixes first,
  typed errors second, SQL correctness third, structural refactor last.

## 1. Bounded recursion

One shared pattern for all four recursive descent parsers:

- SDBL parser: add `depth: usize` to the parser state, incremented in
  `parse_primary` on `(` and in unary chains; limit 128. Exceeding it
  returns a `QueryDiagnostic` with kind `TooDeep` at the offending token.
- Left-associative binary construction uses a separate query-wide budget of
  4,096 operators. Every `OR`, `AND`, comparison, additive, and
  multiplicative node consumes that budget, so iterative parsing cannot build
  an unbounded recursively compiled or dropped tree.
- SQL generation walks the left spine of binary trees iteratively and appends
  the equivalent nested SQL in linear order. The maximum accepted 4,096-node
  boundary is an executable invariant rather than an assumption about thread
  stack size.
- Metadata parsers (`value.rs`, `db_names.rs`, `config.rs`): thread a depth
  counter through `value()`/`list()`; limit 512 (the format nests deeply in
  real configurations, but far below thousands). Exceeding it returns
  `MetadataError` with the current byte offset.
- Compilation walks the same `Expression` tree the parser built. The recursive
  nesting limit and query-wide binary-node budget jointly bound
  `compile_expression` and recursive `Drop`.

## 2. Metadata resolution totality

- `recase_postgres_identifier`: iterate over `char_indices()` instead of
  byte offsets; non-ASCII characters pass through unchanged (no Latin-1
  `char::from(byte)` mojibake). Property: output is always valid UTF-8 and
  ASCII-only inputs behave exactly as before.
- `resolve_metadata*` returns `(MetadataSnapshot, ResolutionReport)` (or a
  `ResolvedMetadata` struct). `ResolutionReport` lists typed findings:
  `DescriptorMissing`, `TableNotLive`, `UnknownColumnTag`, `DuplicateGuid`,
  each with the object/table name. Nothing panics; nothing is silently
  dropped. The CLI prints the report at `metadata` verbosity.
- `schema.rs` unknown column tag: keep the table, keep the column with a
  `ColumnType::Unknown(tag)` variant (or skip only the column), and record
  a report finding — never drop the table.

## 3. Typed diagnostics

- `QueryDiagnostic` gains `kind: QueryDiagnosticKind` (`#[non_exhaustive]`
  enum: `Lex`, `Syntax`, `TooDeep`, `UnknownObject`, `AmbiguousObject`,
  `UnknownField`, `AmbiguousField`, `NotLive`, `UnsupportedFeature`,
  `PresentationPlan`, `PresentationBatch`, `Metadata`, …) plus a `kind()`
  accessor and `Error::source()` for wrapped `Diagnostic`/`LookupError`.
- Every construction site supplies its kind explicitly; public categories do
  not depend on diagnostic wording or user-controlled names. Parser EOF
  diagnostics use the calculated end-of-input position.
- `From<Diagnostic>` maps the lexer kind directly instead of formatting and
  re-parsing the display string.
- Metadata diagnostics take the span of the token that triggered
  resolution (always available at the call sites; the synthetic
  `"presentation lookup"` token is replaced by an explicit constructor).
- `MetadataError` gains `kind: MetadataErrorKind` and documents the offset
  unit per kind (bits for DEFLATE, bytes for parsers).

## 4. SQL correctness

- `quote_identifier` moves into `SqlDialect`: PostgreSQL keeps `"…"` with
  doubling; MSSQL emits `[…]` with `]` → `]]`. Internal aliases are emitted
  through the dialect directly; generated SQL is never rewritten as text.
- Join deduplication: one key — `(source alias, source field, target
  object, database_type)` — implemented once and used by both dereference
  and presentation paths, so a type-guarded presentation join is never
  reused for an unguarded dereference (or vice versa).
- Output labels: a per-branch label allocator truncates to the dialect
  limit (63 bytes for PostgreSQL, 128 UTF-16 code units for MSSQL) on a UTF-8 boundary
  and appends a numeric suffix on collision; `CompiledQuery.columns` always
  matches the labels actually emitted.
- `MsSqlBackend::new(year_offset)` returns `Result` (accepted range
  `0..=10_000`); date arithmetic uses `checked_add`. The old `const fn` is
  kept as `MsSqlBackend::default()` for offset 0.
- Empty-string literals go through `dialect.string_literal("")` everywhere
  (fixes the `''` vs `N''` inconsistency in `wrap_reference_presentation`).
- PostgreSQL string literal generation assumes the server default
  `standard_conforming_strings = on`; quotes are doubled while backslashes are
  emitted unchanged.

## 5. Structural refactor of `src/query`

Split `core.rs` (internal modules only; the public facade in `query.rs` is
unchanged except for the trait):

- `diag.rs` — `QueryDiagnostic`, kinds, conversions.
- `ast.rs` — tokens-borrowing AST.
- `parser.rs` — recursive descent with the depth budget.
- `dialect.rs` — `SqlDialect` including quoting and label limits.
- `resolve.rs` — snapshot lookups (`find_metadata_object`,
  `queryable_fields`, catalog index) — compilation uses a per-compilation,
  demand-populated catalog keyed by the physical table name. It projects only
  objects reached by the query, reuses each projected field set through
  `Arc<[QueryableField]>`, and remains correct for malformed snapshots with
  duplicate object GUIDs without an O(N) identity scan.
- `codegen/` — the compiler; `CompilationContext` and `JoinedContext`
  collapse into one context over `Vec<SourceScope>` (a single-source query
  is the one-scope case), removing the duplicated `resolve_dereference`,
  `ensure_presentation_join`, expression compilers, and label helpers. The
  single-source and JOIN paths share projection, aggregate, filter, ordering,
  and result assembly; only relation construction differs. The `codegen`
  files are real Rust modules with explicit imports rather than textual
  `include!` fragments.

Backend abstraction:

```rust
pub trait Backend: sealed::Sealed + Copy {
    fn dialect(&self) -> SqlDialect; // pub(crate)
}
pub struct Prepared<B: Backend> { /* source + backend + request */ }
```

The eight near-identical entry points become one generic implementation on
`QueryCompiler<B>`; `PreparedPostgresQuery`/`PreparedMsSqlQuery` become
aliases of `Prepared<B>` (kept as deprecated aliases for one release).
`prepare` keeps the parsed AST is not storable (borrowed tokens), but it
must not throw away the compilation: it stores the compiled SQL skeleton or
at minimum documents the recompile; the chosen design: `Prepared` caches
the `PresentationRequest` and recompiles on `compile()` — renamed docs make
the cost explicit.

Performance in the same pass: `names_equal` compares
`char`-by-`char` with `to_lowercase` iterators (no `String` allocation) and
gets an ASCII fast path; `resolve_named_field` returns `&QueryableField`;
join keys compare object identity, not generated SQL text.

## 6. DEFLATE decoder

- Accept dynamic blocks whose distance tree is empty (RFC 1951 permits
  HDIST=1 with a zero-length code when the block contains no matches);
  reject only if a match actually references a distance.
- Reuse one `Vec<HuffmanEntry>` per inflate call (clear + refill) instead
  of allocating up to 128 KiB per tree per block.
- `output` growth uses `try_reserve`-style checks against the output limit
  before doubling; stored blocks and non-overlapping back-references copy
  with `extend_from_within`/`extend_from_slice`.
- New error-path tests cover all sixteen diagnostics; a `cargo-fuzz` target
  (`fuzz/fuzz_targets/inflate.rs`, separate crate, not a workspace default
  member) exercises `inflate_raw_deflate_bounded`.

## 7. Lexer

- Move the implementation to `src/lexer.rs`; `lib.rs` keeps
  `pub use lexer::*` so paths do not change.
- `Keyword`, `TokenKind`, `DiagnosticKind` become `#[non_exhaustive]`.
- `keyword()` matches without allocating: length pre-check plus
  case-insensitive `char` comparison against both spellings.
- `impl Iterator for Lexer` yielding `Result<Token, Diagnostic>`; after the
  first `Err` the iterator fuses to `None`.

## 8. Test strategy

- Backend parity: rewrite `tests/query_compile.rs` feature and rejection
  tests over a `for_each_backend!` macro so every scenario runs on both
  dialects; MSSQL golden strings added where output legitimately differs.
- Table-driven lexer test fixing all 46 bilingual keyword spellings, plus
  `UnexpectedCharacter`, decimal numbers, empty input, BOM, CRLF, and a
  span/lexeme round-trip check.
- Resolution mismatch fixtures: declared-not-live, live-not-declared,
  unknown column tag, duplicate GUIDs — asserting `ResolutionReport`
  contents.
- Presentation batch boundaries: 0, 1, 1,024, and 1,025 references.
- Depth-limit tests: 5,000 nested parentheses and 5,000 nested braces
  return diagnostics quickly (no abort).
- Dev-dependencies: `proptest` for lexer/recase/inflate round-trips is
  optional and may be deferred; the fuzz crate lives outside the workspace
  default members so `cargo test --workspace` stays dependency-light.
