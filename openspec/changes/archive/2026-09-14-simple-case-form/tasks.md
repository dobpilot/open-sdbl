## 1. Implementation

- [x] 1.1 Parse the subject of the simple form into the `ВЫБОР` node.
- [x] 1.2 Render every alternative as the comparison of the subject with
  its value, reusing the comparison rules.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a number, a reference, a type value, a
  composite field, a field alternative, `NULL` and a predicate position;
  goldens for the rendered shape.
- [x] 2.2 Re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
