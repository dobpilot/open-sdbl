## 1. Implementation

- [x] 1.1 Add the `ТИП`/`TYPE`, `ТИПЗНАЧЕНИЯ`/`VALUETYPE`, and
  `НЕОПРЕДЕЛЕНО`/`UNDEFINED` keywords, the `TypeLiteral` and
  `ValueType` expression nodes, and their parsing.
- [x] 1.2 Add `ColumnKind::Type` and `ColumnKind::Undefined` and the
  public `TypeValue` codec.
- [x] 1.3 Compile `ТИПЗНАЧЕНИЯ` for composite fields, payload columns,
  fixed kinds, literals, and parameters; the member predicates for
  comparisons with `ТИП`; `ТИП` literals; the undefined literal and
  its comparisons; the diagnostics; the fingerprint, aggregate, and
  join-scope walks; source-free statements.
- [x] 1.4 Render `Type` cells by name in the console; extend
  completion.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: projection of the three, member
  predicates, `В (ТИП…)`, two-field comparison, nested-query payload,
  parameter, undefined comparisons, diagnostics.
- [x] 2.2 Run the platform's type probes through the console on the
  probe base and compare rows.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
