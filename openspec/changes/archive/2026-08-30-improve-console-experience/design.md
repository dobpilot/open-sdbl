## Context

The current asynchronous byte reader is appropriate for pipes but terminals
need cursor movement and history. Query compilation is already separately
timed at the call boundary and the resulting SQL is available before the
read-only transaction begins.

## Decisions

### Use a line editor only for TTY input

Interactive stdin uses `rustyline` with in-memory history. Completed queries
and metadata commands are added to the current session history. Piped input
continues through the existing byte-oriented Tokio reader, preserving UTF-8
recovery and automation behavior.

### Keep `repl` as an undocumented compatibility alias

Help and documentation advertise `console`. Accepting the former spelling
avoids breaking scripts while moving the user-facing vocabulary to console.

### Show compilation observability before execution

Measure only `compile_postgres_query`, format the duration with a compact
nanosecond/microsecond/millisecond unit, and print both duration and exact SQL
before sending it to PostgreSQL. Compilation failures report elapsed time but
have no SQL to display.

### Render a top-level command footer

Before every top-level interactive prompt, print a compact line covering
`\dt`, `\di`, `\d`, `\refresh`, `\help`, and `\q`. Do not repeat it between
lines of one multiline statement.

## Risks / Trade-offs

- History is intentionally scoped to the current process and does not write a
  history file containing potentially sensitive query literals.
- The terminal readline call is blocking, but the CLI performs no concurrent
  work while awaiting a user command.
