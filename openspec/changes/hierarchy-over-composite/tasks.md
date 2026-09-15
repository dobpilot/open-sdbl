## 1. Compilation

- [x] 1.1 Accept a composite reference in `В ИЕРАРХИИ`: compare the
  identifier member with the seeds and guard the type member.
- [x] 1.2 Keep the diagnostic for a value that is not a reference.
- [x] 1.3 Give an untyped `NULL` branch of an alternative the type of that
  alternative: the query this change unlocks nests a `ВЫБОР` of unbound
  parameters, which PostgreSQL reads as `text` and then refuses beside a
  reference payload.

## 2. Verification

- [x] 2.1 Compare the probe answers with the platform for a hierarchical
  seed and for seeds of a catalog without hierarchy.
- [x] 2.2 Re-record the corpus, execute on the live base, and update the
  documentation.
- [x] 2.3 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
