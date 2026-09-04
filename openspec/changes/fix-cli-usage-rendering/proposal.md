## Why

The CLI routes every top-level error through the terminal field escaper. This
is correct for external error text, but it also converts the trusted newlines
in built-in usage/help output into visible `\n` sequences, making ordinary
argument errors unreadable.

## What Changes

- Render trusted `CliError::Usage` layout with its real line breaks.
- Continue escaping every other top-level error as untrusted terminal text.
- Add an executable-level regression test for the plaintext opt-in error.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: usage diagnostics preserve readable help layout without
  weakening escaping for database-, metadata-, or parser-derived errors.

## Impact

Only top-level CLI error presentation changes. Exit codes, accepted arguments,
and the `--insecure-plaintext` requirement are unchanged.
