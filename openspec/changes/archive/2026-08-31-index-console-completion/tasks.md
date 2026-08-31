## 1. Snapshot-scoped field catalog

- [x] 1.1 Add a queryable-field catalog derived from the snapshot's current
  public vectors.
- [x] 1.2 Index current fields by owner table and number only for that catalog
  build, preserving first-match semantics.

## 2. Linear completion construction

- [x] 2.1 Cache queryable fields once per live named object.
- [x] 2.2 Resolve reference targets through a normalized table map.
- [x] 2.3 Deduplicate candidates through normalized hash membership while
  preserving candidate content and final order.
- [x] 2.4 Add regression coverage for candidate parity and uniqueness.

## 3. Verification

- [x] 3.1 Rebuild release and verify the reported live console reaches its
  prompt through SOCKS5.
- [x] 3.2 Measure live startup against the greater-than-120-second baseline.
- [x] 3.3 Run all repository quality gates and strict OpenSpec validation.
