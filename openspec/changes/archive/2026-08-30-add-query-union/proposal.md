## Why

The lexer recognizes `ОБЪЕДИНИТЬ`/`UNION` and `ВСЕ`/`ALL`, but the
bounded query parser rejects them as trailing syntax. Users cannot combine
independently filtered metadata sources into one read-only result.

## What Changes

- Parse two or more SELECT branches separated by `ОБЪЕДИНИТЬ`/`UNION` or
  `ОБЪЕДИНИТЬ ВСЕ`/`UNION ALL`.
- Compile every branch independently through authoritative metadata and join
  resolution.
- Require compatible logical and expanded SQL projection widths and keep the
  first branch's output labels.
- Apply final ordering to the combined result.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: extend the bounded SELECT grammar with query unions.

## Impact

- Dependency-free parser and PostgreSQL generator in `open-sdbl`.
- Console execution remains one generated, read-only PostgreSQL statement.
