## 1. Implementation

- [x] 1.1 Describe a 1C value as the members a composite field stores it
  in: the type tag, the member that carries it, and the reference table
  number of a reference.
- [x] 1.2 Render `=`, `<>` and `В (…)` of a composite field from that
  description, synthesizing the `RTRef` member when the field has none.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a reference, a string, a number, a date, a
  boolean, a field operand and a list; goldens on both dialects.
- [x] 2.2 Re-record the demo corpus.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the five
  CI checks and strict OpenSpec validation.
