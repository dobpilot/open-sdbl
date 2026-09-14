## 1. Implementation

- [x] 1.1 Decode the criterion's name and content from its Config
  resource and expose them on the snapshot.
- [x] 1.2 Parse `КритерийОтбора.<Имя>(<значение>)` as a source.
- [x] 1.3 Compile it as the union of one selection per content field.

## 2. Verification and documentation

- [x] 2.1 Platform probes for a projection, a dereference, a grouping by
  type and a join, on a probe configuration that declares a criterion.
- [x] 2.2 Goldens; extend the demo fixture with the criteria and
  re-record; update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
