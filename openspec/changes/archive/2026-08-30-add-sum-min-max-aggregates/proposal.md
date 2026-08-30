# Change: Add SUM, MIN, and MAX aggregates

## Why

COUNT is supported, but the other basic aggregate projections requested by
query authors are rejected as field names followed by unsupported syntax.

## What Changes

- Generalize the internal COUNT projection into a bounded aggregate node.
- Add bilingual `SUM`/`СУММА`, `MIN`/`МИНИМУМ`, and `MAX`/`МАКСИМУМ`.
- Preserve `COUNT(DISTINCT field)` / `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ поле)`.
- Continue returning aggregate results as text and rejecting unsafe FULL JOIN
  aggregation or non-aggregate projected fields without GROUP BY.

## Impact

- Affected specs: `sdbl-lexer`, `query-repl`.
- Affected code: lexer, parser, SQL compiler, completion, tests, and docs.
