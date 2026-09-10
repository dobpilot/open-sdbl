## 1. SQL generation

- [x] 1.1 Classify join equality operands (fixed, payload, compound) and
  widen fixed and compound sides against a payload column, keeping the
  widened expression as the join marker.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: fixed field joined to a temporary
  table payload column, composite field joined to a derived payload column,
  payload joined to payload, and a `ПОЛНОЕ СОЕДИНЕНИЕ` anchored on a widened
  equality.
- [x] 2.2 Update `docs/query-language-support.md` and run formatting,
  Clippy, workspace tests, rustdoc, and strict OpenSpec validation.
