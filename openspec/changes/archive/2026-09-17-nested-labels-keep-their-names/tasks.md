## 1. Implementation

- [x] 1.1 `CompiledColumn::named` carries the requested alias; the
  projection compilers pass it; `derived_field` names the field by it.

## 2. Verification

- [x] 2.1 Test with an alias over 63 bytes on a nested query; corpora
  rerecorded; the five CI checks.
