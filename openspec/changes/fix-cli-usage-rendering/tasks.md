## 1. Error rendering

- [x] 1.1 Add a structured top-level error renderer that preserves layout only
  for the trusted `Usage` variant and escapes all other variants.
- [x] 1.2 Route `async_main` through the renderer without changing exit codes or
  broken-pipe handling.
- [x] 1.3 Represent the missing PostgreSQL plaintext opt-in as a dedicated
  diagnostic with a stable code, concise cause, and exact remediation hint.

## 2. Verification

- [x] 2.1 Add a CLI regression test proving the plaintext opt-in diagnostic
  contains real line breaks and no literal `\n` sequences.
- [x] 2.2 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, cargo audit, and strict OpenSpec validation.
- [x] 2.3 Assert the complete diagnostic output so its machine-readable code and
  human-readable guidance remain stable.
