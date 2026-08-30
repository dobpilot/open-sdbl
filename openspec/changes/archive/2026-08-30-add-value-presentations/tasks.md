## 1. Indexed metadata identities

- [x] 1.1 Add GUID-backed object and attribute IDs, numeric standard-field IDs,
  and typed lookup errors.
- [x] 1.2 Build immutable metadata indexes during snapshot resolution.
- [x] 1.3 Expose object, attribute, field, and database-type lookup methods.

## 2. Presentation planning and SQL compilation

- [x] 2.1 Add bilingual presentation functions and the reference
  `.Представление`/`.Presentation` property.
- [x] 2.2 Add an ID-only batch request/response protocol and safe structured
  presentation-expression AST.
- [x] 2.3 Compile source, fixed-target, multi-target, and scalar presentation
  expressions with validated plans.
- [x] 2.4 Preserve presentation behavior through JOIN, FULL JOIN transposition,
  and UNION compilation.

## 3. CLI integration and verification

- [x] 3.1 Implement the CLI presentation-plan provider and bounded
  `moka::future::Cache` outside the core crate.
- [x] 3.2 Add lexer, metadata lookup, compiler, provider, cache, and diagnostic
  tests.
- [x] 3.3 Verify generated presentation SQL against PostgreSQL information base
  `test` using a read-only transaction.
- [x] 3.4 Update documentation, verify Rust Edition 2024, and pass formatting,
  Clippy, tests, rustdoc, and strict OpenSpec validation.
