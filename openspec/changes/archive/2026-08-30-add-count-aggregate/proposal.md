# Change: Add COUNT aggregate projections

## Why

The console rejects the basic query `SELECT COUNT(*) FROM ...` because function
calls other than presentation functions are not represented in the projection
AST.

## What Changes

- Add bilingual `COUNT`/`КОЛИЧЕСТВО` projection syntax.
- Support `COUNT(*)`, `COUNT(field)`, and `COUNT(DISTINCT field)`.
- Return aggregate values as text for the existing CLI row transport.
- Reject COUNT in transposed FULL JOIN branches until aggregation can be
  applied once above the complete transposed relation.

## Impact

- Affected specs: `sdbl-lexer`, `query-repl`.
- Affected code: lexer, parser, PostgreSQL compiler, completion, and tests.
