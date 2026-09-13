## 1. Implementation

- [x] 1.1 Parse `НЕ` on its own level between the conjunction and the
  comparison, leaving the sign operators where they are.

## 2. Verification and documentation

- [x] 2.1 Platform probes for `НЕ` before a comparison, a conjunction, a
  disjunction, `ПОДОБНО`, `В`, `ССЫЛКА`, `ЕСТЬ NULL` and `В ИЕРАРХИИ`;
  goldens for the parsed shape.
- [x] 2.2 Re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
