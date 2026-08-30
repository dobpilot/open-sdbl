## 1. Totals resolution

- [x] 1.1 Resolve `AccumRgT` by object GUID from DBNames and validate the
  declared/live totals shape.
- [x] 1.2 Add metadata/compiler regressions that reject missing or mismatched
  totals without physical-name guessing.

## 2. Balance compiler

- [x] 2.1 Generate the current Balance relation exclusively from the latest
  totals period, merging split rows and preserving virtual/outer filters.
- [x] 2.2 Generate historical Balance from a totals anchor plus the bounded
  signed movement delta.
- [x] 2.3 Keep Turnovers movement-based and preserve aliases, JOINs,
  dereferences, zero suppression, and diagnostics.

## 3. Verification

- [x] 3.1 Compare current and historical generated balances with independent
  read-only calculations on PostgreSQL `test`.
- [x] 3.2 Update documentation, pass all repository gates, and archive the
  strict OpenSpec change.
