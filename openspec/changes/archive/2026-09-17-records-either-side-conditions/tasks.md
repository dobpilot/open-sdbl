## 1. Implementation

- [x] 1.1 `compile_accumulation_condition` compiles each condition over
  the base fields and over the mirror, `OR`-ing the two when they differ;
  the joins of a two-sided name are retired between the readings.
- [x] 1.2 `ДвиженияССубконто` passes the debit and credit readings of
  `Счет`, the non-balance fields and the extra dimensions.

## 2. Verification

- [x] 2.1 Test on the buh fixture; the SQL executed on the live base;
  corpora rerecorded; the language table; the five CI checks.
