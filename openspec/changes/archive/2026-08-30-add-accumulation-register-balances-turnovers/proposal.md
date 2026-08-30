# Change: Add accumulation-register Balances and Turnovers

## Why

The compiler can query accumulation-register movement rows but cannot express
the standard aggregated `Остатки` or `Обороты` read paths.

## What Changes

- Classify accumulation-register dimensions, resources, and attributes from
  authoritative Config collection GUIDs.
- Accept bilingual `.<Остатки|Balance>` and `.<Обороты|Turnovers>`
  virtual sources with a bounded parameter subset.
- Aggregate active movement rows by dimensions and data separators, applying
  receipt/expense direction for balance registers.
- Expose resources with standard `Остаток`/`Balance` and
  `Оборот`/`Turnover` suffixes while retaining reference metadata on
  dimensions.

## Impact

- Affected specs: `onec-metadata`, `query-repl`, `sdbl-lexer`.
- Affected code: Config resolution, lexer, source AST/compiler, console
  completion, tests, and documentation.

