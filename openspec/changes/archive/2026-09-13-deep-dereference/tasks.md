## 1. Implementation

- [x] 1.1 Walk the path hop by hop, joining each target to the previous
  alias and reusing identical joins.
- [x] 1.2 Refuse a walk through a composite reference.

## 2. Verification and documentation

- [x] 2.1 Goldens for two and three hops, a shared prefix, a path in
  `ГДЕ`, `СГРУППИРОВАТЬ ПО` and `УПОРЯДОЧИТЬ ПО`, and the composite
  refusal; probe the platform for all of them.
- [x] 2.2 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
