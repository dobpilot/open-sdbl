## 1. Implementation

- [x] 1.1 Add the document-journal kind with its alias, physical prefix
  and query names.
- [x] 1.2 Name the journal's reference column `Ссылка` and resolve `Тип`
  as the type value of that reference.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a projection, `Тип`, a dereference, a filter
  and a join; goldens on both dialects.
- [x] 2.2 Extend the demo fixture with the journals the corpus reads and
  re-record it; update README and `docs/query-language-support.md`; run
  the five CI checks and strict OpenSpec validation.
