## 1. Syntax and compilation

- [x] 1.1 Add bilingual COUNT keyword and completion.
- [x] 1.2 Parse wildcard, field, and DISTINCT field COUNT arguments.
- [x] 1.3 Compile COUNT for ordinary and native JOIN branches with text output.
- [x] 1.4 Reject unsafe compound fields and transposed FULL JOIN aggregation.

## 2. Verification

- [x] 2.1 Add lexer/compiler regressions for Russian and English forms.
- [x] 2.2 Execute the reported catalog count against PostgreSQL `test`.
- [x] 2.3 Pass quality gates and archive the change.
