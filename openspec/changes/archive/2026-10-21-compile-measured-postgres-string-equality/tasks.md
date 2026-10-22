# Tasks

- [x] Reproduce native string equality SQL and incompatible scalar comparisons.
- [x] Implement the bounded PostgreSQL equality correction.
- [x] Run fmt, Clippy, workspace tests, rustdoc and strict OpenSpec validation.
- [x] Archive and deliver version 0.9.36 with SQL Server limitation.

Evidence: `docs/research/string-comparison-0936/` records red reproduction, own regression tests, exact reviewed SQL-shape patches and corpus wrapper-only classification. Native controls are retained unchanged in `tests/fixtures/string-comparison-native-0936/`.
