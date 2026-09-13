## 1. Implementation

- [x] 1.1 Parse a join group recursively and flatten it into the branch's
  join list, outer join first.
- [x] 1.2 Refuse a grouping whose flat form would answer differently.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a group of left joins, a nested inner join,
  an inner outer join and three levels; goldens for the flat shape.
- [x] 2.2 Re-record the demo corpus; update
  `docs/query-language-support.md`; run the five CI checks and strict
  OpenSpec validation.
