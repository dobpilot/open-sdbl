## Why

An expert review of the library found three classes of defects:

1. **Crash paths on untrusted input.** The query parser (`parse_primary` →
   `parse_or`) and all three metadata parsers (`value.rs`, `db_names.rs`,
   `config.rs`) recurse without a depth limit, so `((((…` or `{{{{…` aborts
   the process with a stack overflow that `catch_unwind` cannot intercept —
   contradicting the crate's "bounded query parsing" promise.
   `recase_postgres_identifier` slices a `&str` on a non-char boundary and
   panics on any non-ASCII live-catalog identifier, and `resolve_metadata`
   cannot report it because it returns no `Result`. `MsSqlBackend::new`
   accepts any `i32`, allowing arithmetic overflow panics during compilation.
2. **Correctness bugs.** Join deduplication uses different keys in
   `resolve_dereference` (source field only) and `ensure_presentation_join`
   (field + target + database type), so a dereference can silently reuse a
   presentation join carrying a foreign type guard. An unknown SchemaStorage
   column tag silently drops the whole table. MSSQL identifiers are quoted
   with `"…"`, which is only valid under `QUOTED_IDENTIFIER ON`. Result
   column labels ignore PostgreSQL's 63-byte truncation and can collide.
3. **Structural debt.** `src/query/core.rs` holds 6,523 lines mixing DTOs,
   dialects, parsing, metadata resolution, and code generation, with ~800
   duplicated lines between the single-source and joined compilation
   contexts (the dedup bug above is a direct consequence). Diagnostics are
   bare `String`s without machine-readable kinds; metadata errors fabricate
   position `1:1`. Error-path test coverage is ~1/3, MSSQL is tested five
   times less than PostgreSQL, and the hand-written DEFLATE decoder has no
   fuzzing and one covered error branch out of sixteen.

## What Changes

- Enforce recursion/depth limits in the SDBL parser and the three metadata
  parsers; malformed depth becomes a positional diagnostic, never an abort.
- Fix the non-ASCII recase panic; make `resolve_metadata` return typed
  resolution reports instead of silently degrading objects.
- Validate `MsSqlBackend` year offsets at construction; use checked
  arithmetic in date generation.
- Unify join-deduplication keys across dereference and presentation paths.
- Introduce machine-readable `QueryDiagnosticKind` and `MetadataErrorKind`
  while keeping `Display` output stable where practical; carry real source
  spans for metadata-phase diagnostics.
- Make identifier quoting a dialect primitive (`[…]` for MSSQL, `"…"` for
  PostgreSQL); deduplicate and bound generated column labels.
- Merge the duplicated single/joined compilation contexts, split
  `query/core.rs` into focused submodules, add a sealed `Backend` trait with
  a generic `Prepared<B>`, and reuse the existing field catalog index inside
  compilation.
- Preserve SchemaStorage tables containing unknown column tags; accept
  RFC-1951 dynamic blocks with an empty distance tree; bound per-block table
  work in the DEFLATE decoder.
- Move the lexer out of `lib.rs` into `src/lexer.rs` (re-exported), mark
  public enums `#[non_exhaustive]`, provide an `Iterator` adapter, and drop
  the per-identifier `to_uppercase` allocation.
- Close the test gaps: all DEFLATE error branches plus a fuzz target,
  dialect-parity test macros, the full bilingual keyword table, lexer
  `UnexpectedCharacter`, resolution-mismatch fixtures, and presentation
  batch boundaries (0 and 1,025 references).

## Capabilities

### New Capabilities

- `query-compilation`: bounded parsing, typed diagnostics with accurate
  positions, dialect-correct quoting, deterministic join reuse, and unique
  result labels for SDBL-to-SQL compilation.

### Modified Capabilities

- `sdbl-lexer`: unexpected-character diagnostics become a specified
  behavior; tokens are additionally reachable through the standard iterator
  protocol.
- `onec-metadata`: decoding and resolution become total over malformed and
  non-ASCII input — bounded recursion, tolerant column-tag handling,
  RFC-1951 conformance, and explicit resolution mismatch reporting.
- `crate-architecture`: query compilation is reachable through one sealed
  backend trait so applications can write backend-generic code.

## Impact

- Public API additions: diagnostic `kind()` accessors, `Backend` trait,
  `Prepared<B>`, lexer iterator, resolution report type. Breaking changes:
  `#[non_exhaustive]` on public enums, `MsSqlBackend::new` becomes
  fallible, `resolve_metadata` returns a report alongside the snapshot.
- No new production dependencies for the core crate. Dev-dependencies may
  add a fuzz target (separate `fuzz/` crate) and property tests.
- Generated SQL changes only where it was wrong: MSSQL identifier quoting,
  join reuse with type guards, and label truncation handling.
