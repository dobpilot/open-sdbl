## 1. Crash-path elimination (do first; each task lands green)

- [x] 1.1 Add a depth budget (limit 128) to the SDBL parser covering nested
  parentheses and unary chains; exceeding it returns a positional
  `QueryDiagnostic`. Bound left-associative binary trees to 4,096 operators
  per query. Tests: 5,000 nested parentheses, functions, unary operators, and
  oversized arithmetic/logical chains produce diagnostics, not stack
  overflows; exactly 4,096 binary operators compile successfully.
- [x] 1.2 Add a depth budget (limit 512) to the three metadata parsers
  (`src/metadata/value.rs`, `db_names.rs`, `config.rs`); exceeding it
  returns `MetadataError` at the current offset. Test: 5,000 nested braces
  in each parser.
- [x] 1.3 Rewrite `recase_postgres_identifier` over `char_indices()` so
  non-ASCII identifiers never panic and pass through unchanged. Tests:
  Cyrillic table/column names, mixed ASCII/non-ASCII, plus the existing
  five recase fixtures unchanged.
- [x] 1.4 Make `MsSqlBackend::new` fallible (accepted offset range
  `0..=10_000`), add `MsSqlBackend::default()` for offset 0, and use
  checked arithmetic in `DATETIME`/`DATEADD` generation. Tests: rejected
  extreme offsets; `i32::MAX`/`i32::MIN` no longer reachable.
- [x] 1.5 Unify join-deduplication keys: one shared key
  `(source alias, source field, target object, database type)` used by both
  `resolve_dereference` and `ensure_presentation_join` in both compilation
  contexts. Test: a query that dereferences and presents the same
  multi-target field produces two distinct joins (or one join only when the
  type guard matches), with golden SQL for both dialects.

## 2. Typed diagnostics

- [x] 2.1 Add `QueryDiagnosticKind` (`#[non_exhaustive]`) and
  `QueryDiagnostic::kind()`; map every construction site explicitly without
  message-text classification; implement
  `Error::source()`. Existing message-based tests keep passing.
- [x] 2.2 Replace the `From<Diagnostic>` string round-trip with a direct
  kind/position mapping.
- [x] 2.3 Thread real token spans into metadata-phase diagnostics; remove
  `QueryDiagnostic::metadata()`'s fabricated `1:1` position and the
  synthetic `"presentation lookup"` token. Test: an unknown-object error
  points at the object token and an EOF error points at the real end of input.
- [x] 2.4 Add `MetadataErrorKind` with documented offset units (bits for
  DEFLATE, bytes for parsers) and a `kind()` accessor; replace hardcoded
  `offset = 0` sites with the actual parser offset.

## 3. Metadata resolution totality

- [x] 3.1 Introduce `ResolutionReport` with typed findings
  (`DescriptorMissing`, `TableNotLive`, `UnknownColumnTag`,
  `DuplicateGuid`, `IndexMismatch`) and return it from `resolve_metadata*`;
  migrate the CLI to print it. Tests: declared-not-live and
  live-not-declared fixtures assert report contents.
- [x] 3.2 Stop dropping SchemaStorage tables on an unknown column type tag:
  keep the table, record a finding, and represent the column as unknown. Keep
  tables on malformed child counts/reference declarations and report those
  anomalies as well.
  Test: a fixture with a novel tag keeps the table queryable for its known
  columns.
- [x] 3.3 Replace `trim_start_matches('_')` with `strip_prefix('_')` at the
  three physical-name sites; move the `Date_Time` special case into
  `normalize.rs` with a unit test.
- [x] 3.4 Filter enum value collection to the enum-values GUID collection so
  forms/templates are not misread as values; add a fixture test.

## 4. SQL generation correctness

- [x] 4.1 Move `quote_identifier` into `SqlDialect`; MSSQL emits `[…]` with
  `]]` escaping. Emit every internal alias through the dialect without a
  textual SQL post-pass. Update MSSQL golden tests; PostgreSQL output is unchanged.
- [x] 4.2 Bound and deduplicate output column labels per dialect (63 bytes
  PostgreSQL / 128 UTF-16 code units MSSQL, UTF-8-safe truncation, numeric suffix on
  collision); `CompiledQuery.columns` matches emitted labels. Tests: long
  Cyrillic aliases collide before the fix, are distinct after.
- [x] 4.3 Route empty-string literals through `dialect.string_literal("")`
  in `wrap_reference_presentation`; document (or escape) the
  `standard_conforming_strings` assumption for PostgreSQL literals.
- [x] 4.4 Remove the unused `_dialect` parameters or use them where the
  output should be dialect-specific.

## 5. Structural refactor of `src/query`

- [x] 5.1 Split `core.rs` into `diag.rs`, `ast.rs`, `parser.rs`,
  `dialect.rs`, `resolve.rs`, and `codegen/` without behavior changes
  (golden tests are the safety net); no file exceeds ~1,500 lines.
- [x] 5.2 Collapse `CompilationContext` and `JoinedContext` into one context
  over source scopes, deleting the duplicated `resolve_dereference`,
  `ensure_presentation_join`, expression compilers, and `output_label`
  copies.
- [x] 5.3 Add a sealed `Backend` trait and generic `Prepared<B>`; collapse
  the eight entry points into one generic implementation; keep deprecated
  type aliases for the old prepared-query names.
- [x] 5.4 Use the indexed `queryable_field_catalog` inside compilation
  instead of recomputing `queryable_fields` per dereference; make
  `resolve_named_field` return a reference; compare join targets by object
  identity rather than generated SQL text.
- [x] 5.5 Make `names_equal` allocation-free (ASCII fast path plus
  `char`-wise case folding) and reconcile it with the
  `eq_ignore_ascii_case` sites so one comparison rule applies everywhere.

## 6. DEFLATE decoder hardening

- [x] 6.1 Accept dynamic blocks with an empty distance tree; error only when
  a match actually needs a distance code. Test: a stream compressed with
  literal-only dynamic blocks inflates.
- [x] 6.2 Reuse the Huffman table allocation across blocks and copy stored
  blocks/back-references with slice operations; enforce the output limit
  before growth doubling.
- [x] 6.3 Cover all sixteen DEFLATE error diagnostics with tests, including
  `back-reference out of range`, truncated streams, reserved block type,
  and oversubscribed trees; assert offsets are meaningful.
- [x] 6.4 Add a `cargo-fuzz` target for `inflate_raw_deflate_bounded` in a
  `fuzz/` crate outside the default workspace members.

## 7. Lexer polish

- [x] 7.1 Move the lexer from `src/lib.rs` to `src/lexer.rs` with re-exports
  preserving all public paths; `lib.rs` keeps only module wiring and crate
  docs.
- [x] 7.2 Mark `Keyword`, `TokenKind`, and `DiagnosticKind`
  `#[non_exhaustive]`.
- [x] 7.3 Implement `Iterator` for `Lexer` yielding
  `Result<Token, Diagnostic>`, fused after the first error; port `tokenize`
  onto it.
- [x] 7.4 Make keyword recognition allocation-free (no `to_uppercase`
  `String` per identifier) while staying case-insensitive for both
  languages.
- [x] 7.5 Add lexer tests: the full 46-keyword bilingual table,
  `UnexpectedCharacter`, operator/punctuation kinds asserted, decimal
  numbers, empty input, comment at EOF, CRLF, BOM, and a
  span-lexeme round-trip check.

## 8. Test parity and coverage

- [x] 8.1 Run every feature and rejection scenario in
  `tests/query_compile.rs` on both backends via a shared macro; add MSSQL
  golden strings where output differs (slices, turnovers, UNION, FULL
  JOIN, dereference, tabular sections, aggregates, TOP, IN, VALUE).
- [x] 8.2 Add presentation batch boundary tests: 0, 1, 1,024, and 1,025
  references.
- [x] 8.3 Add compiler diagnostic tests keyed by `QueryDiagnosticKind` for
  the currently untested paths (syntax expectations, ambiguous names,
  not-live tables, presentation plan validation).
- [x] 8.4 Exercise `attribute_by_id`, `predefined_value`, and every
  `LookupError` variant in metadata tests.
- [x] 8.5 Extract the duplicated test fixtures (`hex`, snapshot builders)
  into a shared test-support module.

## 9. Verification

- [x] 9.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --workspace`, and `cargo doc --no-deps` with warnings denied
  all pass.
- [x] 9.2 `openspec validate harden-core-reliability --strict` passes.
- [x] 9.3 README and crate docs updated for the new `Backend` trait,
  fallible `MsSqlBackend::new`, resolution reports, and diagnostic kinds.
