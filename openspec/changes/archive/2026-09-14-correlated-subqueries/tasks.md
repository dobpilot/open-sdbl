## 1. Implementation

- [x] 1.1 Carry the sources of the enclosing statement into a subquery of
  a predicate, visible only by qualifier and never rendered there.
- [x] 1.2 Name the `EnumOrder` column.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a correlated subquery and for the
  enumeration order; goldens for both, including the refusal in a derived
  source.
- [x] 2.2 Re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
