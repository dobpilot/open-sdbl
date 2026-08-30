## 1. Metadata roles

- [x] 1.1 Decode information-register property purpose from Config collection
  GUIDs and expose it on resolved fields.
- [x] 1.2 Add role-decoding and metadata-resolution regressions.

## 2. SliceLast compilation

- [x] 2.1 Add bilingual lexer, parser, and completion support.
- [x] 2.2 Compile empty, period-bounded, and condition-bounded SliceLast sources
  with authoritative dimension/separator partitions.
- [x] 2.3 Support a SliceLast source in ordinary JOIN paths and diagnose invalid
  kinds, missing Period, and unsupported parameter shapes.

## 3. Verification

- [x] 3.1 Add compiler regressions for pre-slice versus post-slice filtering,
  aliases, joins, and invalid forms.
- [x] 3.2 Execute generated SliceLast SQL against PostgreSQL `test` in a
  read-only session.
- [x] 3.3 Update user documentation, pass all quality gates, and archive the
  validated OpenSpec change.
