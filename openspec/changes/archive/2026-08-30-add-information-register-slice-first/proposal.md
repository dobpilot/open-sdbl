# Change: Add information-register SliceFirst

## Why

The compiler supports the latest slice of a periodic information register but
cannot address its standard earliest-slice counterpart.

## What Changes

- Accept bilingual `.<СрезПервых|SliceFirst>([period][, condition])`
  source syntax.
- Generalize the existing slice AST and PostgreSQL generator around a typed
  earliest/latest direction.
- Select the least eligible Period for every authoritative Config dimension and
  data separator, with virtual parameters applied before the slice.

## Impact

- Affected specs: `query-repl`, `sdbl-lexer`.
- Affected code: lexer, query parser/compiler, console completion, tests, and
  documentation.

