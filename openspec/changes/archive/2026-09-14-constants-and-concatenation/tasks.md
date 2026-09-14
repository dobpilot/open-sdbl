## 1. Implementation

- [x] 1.1 Accept a projection that reads no field in both grouping checks.
- [x] 1.2 Compile `+` over strings as concatenation and refuse a mixed
  operand.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a constant in a grouped statement, a
  constant beside an aggregate, a concatenation and a number beside a
  string.
- [x] 2.2 Goldens; re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
