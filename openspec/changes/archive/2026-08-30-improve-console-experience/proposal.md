## Why

The interactive mode is discoverable only through the implementation-oriented
name `repl`, provides no command reminder or editable history, and hides the
generated SQL and compilation cost. Users need a database-console experience
that makes exploration and compiler behavior visible.

## What Changes

- Make `open-sdbl console postgres` the documented interactive command while
  retaining `repl` as a compatibility alias.
- Display a compact metadata-command hint immediately above each top-level
  interactive prompt.
- Add line editing and current-session history so Up/Down recall prior queries
  and commands.
- Print the generated PostgreSQL SQL and SDBL-to-SQL generation duration before
  database execution.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: rename and improve the interactive console user experience and
  expose query generation details.

## Impact

- Adds `rustyline` only to `open-sdbl-cli`; the core library remains
  dependency-free and performs no terminal I/O.
- Keeps non-interactive piped input supported without line-editor control
  sequences or prompts.
