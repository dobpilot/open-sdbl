## Why

Release 0.3.22 moved `НЕ` to its own precedence level, which added one
stack frame per nesting level of an expression. A query of 128 nested date
functions then exhausted the stack of a test thread and aborted the
process instead of reporting the nesting limit, and the regression went
unnoticed because an aborting process prints no failing-test line.

## What Changes

- The parser SHALL keep a nesting budget that fits a small thread stack:
  the limit drops from 128 to 64 levels, measured against the roughly
  sixteen kilobytes one level of nested date functions costs in a debug
  build.
- The negations of a conjunction SHALL be consumed without a stack frame
  of their own, so the precedence of `НЕ` costs no depth.

## Capabilities

### Modified Capabilities

- `query-compilation`: the nesting budget.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`.
