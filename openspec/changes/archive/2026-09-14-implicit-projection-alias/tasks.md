## 1. Implementation

- [x] 1.1 Accept a projection alias written without `КАК`, sharing one
  rule with the source aliases and denying the clause-opening words.

## 2. Verification and documentation

- [x] 2.1 Platform probes for the projection, source, join, nested-query
  and temporary-table positions, for a contextual keyword as the alias
  and for two words; goldens; re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
