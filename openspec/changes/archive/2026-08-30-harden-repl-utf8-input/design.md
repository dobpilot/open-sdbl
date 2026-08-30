## Context

`tokio::io::AsyncBufReadExt::read_line` validates UTF-8 and returns an error
for the entire operation. Linux canonical TTY editing needs the `IUTF8` flag
to erase a whole multibyte character; the observed PTY defaults to `-iutf8`.

## Decisions

### Guard interactive Linux terminals with `IUTF8`

At REPL startup, inspect stdin with `tcgetattr`. If it is a TTY and `IUTF8` is
unset, enable it with `tcsetattr`. Preserve the complete original `termios` and
restore it when the REPL exits.

### Keep decoding errors local to one input line

Use `read_until(b'\n', Vec<u8>)`, then validate the collected line with
`String::from_utf8`. Invalid bytes produce a diagnostic, clear any pending
multiline statement, and return to the prompt. No fallback encoding is
guessed because the process locale is UTF-8.

## Risks / Trade-offs

- A byte-invalid line cannot be reconstructed reliably, so it is discarded.
- `IUTF8` manipulation is Linux-specific; other platforms still benefit from
  byte-oriented recoverable input.
