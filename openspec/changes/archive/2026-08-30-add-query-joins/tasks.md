## 1. Grammar and resolution

- [x] 1.1 Parse bilingual INNER/LEFT/RIGHT/FULL joins with two metadata sources.
- [x] 1.2 Resolve direct fields, one-hop properties, and equality conjunctions across both sources.

## 2. PostgreSQL generation

- [x] 2.1 Generate native INNER/LEFT/RIGHT and duplicate-safe FULL transpose SQL.
- [x] 2.2 Apply WHERE, DISTINCT, TOP, and ordering at the correct logical scope.
- [x] 2.3 Reject ambiguous aliases, unsupported join conditions, wildcards, and extra joins; accept repeated terminators.

## 3. Verification

- [x] 3.1 Add parser/generator regression and diagnostic tests.
- [x] 3.2 Verify the reported LEFT JOIN plus all other join kinds against the PostgreSQL test infobase.
- [x] 3.3 Update documentation and pass all repository quality gates.
