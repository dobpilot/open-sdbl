## Why

An interactive UTF-8 query can become byte-invalid when a terminal is in
canonical mode with `IUTF8` disabled and Backspace edits a multibyte Cyrillic
character. The current string-based reader terminates the whole REPL instead
of recovering.

## What Changes

- Enable the terminal `IUTF8` input flag for the lifetime of an interactive
  Linux REPL and restore the previous terminal settings on exit.
- Read standard input as bytes and reject a byte-invalid line without closing
  the PostgreSQL session.
- Explain the recoverable input error and accept the next command.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: make UTF-8 terminal editing and malformed input recoverable.

## Impact

- Adds a direct `libc` dependency only to `open-sdbl-cli`; the dependency-free
  `open-sdbl` library remains unchanged.
- Changes terminal flags only for an interactive REPL and restores them using
  an RAII guard.
