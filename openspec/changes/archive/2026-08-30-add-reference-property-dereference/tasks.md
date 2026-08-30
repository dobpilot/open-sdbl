## 1. Core model and parser

- [x] 1.1 Retain the unique SchemaStorage reference target on queryable fields.
- [x] 1.2 Parse local, qualified, and one-hop dereference property paths.
- [x] 1.3 Add positional diagnostics for unsupported or unresolved paths.

## 2. PostgreSQL generation

- [x] 2.1 Build reusable LEFT JOIN plans using source RRef and target ID columns.
- [x] 2.2 Support dereferenced projections, predicates, and ordering.
- [x] 2.3 Add compiler tests for joins, aliases, reuse, and failures.

## 3. Verification

- [x] 3.1 Execute `Организация.Код` against `Справочник.Договоры` in the test IB.
- [x] 3.2 Update documentation and run all workspace/OpenSpec quality gates.
