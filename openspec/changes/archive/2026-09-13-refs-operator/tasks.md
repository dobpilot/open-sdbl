## 1. Implementation

- [x] 1.1 Add the `ССЫЛКА`/`REFS` keyword (contextual identifier), the
  `Refs` expression node, and its parsing at the comparison level.
- [x] 1.2 Compile the type test for composite fields, runtime-typed
  derived columns, and fixed-target fields; raise the `Syntax`
  diagnostics; refuse it in source-free statements; extend the
  fingerprint, aggregate, and join-scope walks.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: composite field, universal
  reference, dereferenced path, nested-query payload, fixed target
  true, wrong target and non-reference diagnostics, use inside `ВЫБОР`,
  `НЕ (… ССЫЛКА …)`, `Т.Ссылка` still a field.
- [x] 2.2 Run the platform's `ССЫЛКА` probes through the console on the
  probe base: composite, fixed-target, dereferenced, and `ГДЕ` uses
  matched row by row, and both incompatible-type probes fail on both
  sides.
- [x] 2.3 Update README, `docs/query-language-support.md`, CLI
  completion; run the five CI checks and strict OpenSpec validation.
