# Change: Add information-register SliceLast

## Why

Queries cannot address the standard `СрезПоследних` virtual table, so callers
must currently reproduce its period and dimension semantics outside SDBL.

## What Changes

- Classify information-register fields as dimensions, resources, or attributes
  from their authoritative Config collection GUID.
- Accept bilingual `.<СрезПоследних|SliceLast>([period][, condition])`
  source syntax.
- Generate a PostgreSQL derived relation over the resolved `_InfoRgN` table,
  selecting the greatest eligible Period for every Config dimension and data
  separator.
- Apply the virtual-table period and condition before the slice and an ordinary
  WHERE after it.

## Impact

- Affected specs: `onec-metadata`, `query-repl`, `sdbl-lexer`.
- Affected code: Config resolution, lexer, query parser/compiler, console
  completion, tests, and documentation.

