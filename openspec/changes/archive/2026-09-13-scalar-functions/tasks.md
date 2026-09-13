## 1. Implementation

- [x] 1.1 Add the keywords as contextual identifiers, the
  `ScalarFunction` node with its arity and kind tables, and the parsing,
  including `Лев`/`Прав` beside the join keywords.
- [x] 1.2 Render every function per dialect and check the argument
  kinds; extend the fingerprint, aggregate, join-scope, and source-free
  walks.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects plus the platform probes for every
  function, including the edge cases and `NULL`.
- [x] 2.2 Update README, `docs/query-language-support.md`, and CLI
  completion; run the five CI checks and strict OpenSpec validation.
