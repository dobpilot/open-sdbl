## Context

`rustyline` exposes a `Helper` composed of `Completer`, `Highlighter`, `Hinter`,
and `Validator`. The existing resolved snapshot contains the authoritative
object and field names needed for context-independent completion candidates.

## Decisions

### Pin the footer with a terminal scroll region

On a TTY, reserve rows `1..height-1` for normal output and render a compact
ASCII command reminder on the final row. Redraw before each input operation to
adapt to terminal resizing. On normal exit, reset the scroll region and clear
the reserved row through an RAII guard.

### Build completion candidates from resolved metadata

Offer console commands, Russian/English query keywords, qualified metadata
object names, and unique field aliases. Match case-insensitively against the
word at the cursor and replace only that word. Rebuild the catalog after
metadata refresh.

### Highlight through the existing lexer

For lexically complete input, color keywords, strings, numbers, parameters,
comments, and known metadata identifiers using ANSI sequences without changing
display width. Incomplete lexical input falls back to the original text so
editing never fails.

## Risks / Trade-offs

- Very small or non-ANSI terminals fall back without a pinned footer.
- Completion is metadata-aware but not yet fully grammar-context-aware, so the
  candidate list may contain valid names that are not applicable at one cursor
  position.
