## Why

The command reminder currently scrolls away with query output, and raw console
input gives no language feedback or discovery while typing. Interactive
metadata exploration needs a stable footer, SDBL highlighting, and completion.

## What Changes

- Reserve the final terminal row as a pinned command footer while the console
  is active and restore the terminal scroll region on exit.
- Add `rustyline` syntax highlighting for SDBL tokens and known metadata names.
- Add tab completion for console commands, bilingual query keywords, resolved
  metadata objects, and queryable fields.
- Refresh completion metadata after `\refresh`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: improve interactive discovery and editing assistance.

## Impact

- CLI-only terminal behavior; piped input and the dependency-free core remain
  unchanged.
- ANSI scroll-region control is enabled only for interactive terminals that
  report at least three rows.
