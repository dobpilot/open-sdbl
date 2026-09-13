## 1. Implementation

- [x] 1.1 Carry an expression or a field path in the ordering term and
  parse either, keeping the term's first token for diagnostics.
- [x] 1.2 Compile expression keys in plain branches, refuse them where
  ordering is positional, and keep the hidden-column path for `ИТОГИ`.

## 2. Verification and documentation

- [x] 2.1 Goldens: expression key, several keys with directions, key with
  `ИТОГИ`, refusals in joined, grouped, union, and source-free branches.
- [x] 2.2 Run the platform's ordering probe on the probe base.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
