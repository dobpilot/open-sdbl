## 1. Console command and input

- [x] 1.1 Add `console` routing/help and retain the `repl` compatibility alias.
- [x] 1.2 Add TTY line editing and current-session query/command history.
- [x] 1.3 Render the compact command hint at every top-level prompt.

## 2. Query observability

- [x] 2.1 Measure and format SDBL-to-SQL compilation duration.
- [x] 2.2 Print the exact generated PostgreSQL SQL before execution.
- [x] 2.3 Add tests for routing, duration formatting, and non-interactive input.

## 3. Verification

- [x] 3.1 Verify console hints, history, SQL output, and execution on the test IB.
- [x] 3.2 Update README and pass workspace/OpenSpec quality gates.
