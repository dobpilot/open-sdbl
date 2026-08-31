## Why

After optimizing DEFLATE and DBNames projection, live profiling attributes the
largest remaining Config-decoding cost to the generic brace parser. It obtains
the current UTF-8 character repeatedly and advances one character at a time
even though the grammar delimiters are ASCII and most strings contain long
delimiter-free spans.

## What Changes

- Scan structural delimiters and atoms by byte offsets over already validated
  UTF-8.
- Copy quoted-string spans between quotes in blocks while preserving doubled
  quote unescaping.
- Keep Unicode whitespace handling, byte-based diagnostics, the public `Value`
  tree, and accepted syntax compatible.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: reduce per-byte overhead when parsing Config and
  SchemaStorage serialized values.

## Impact

- Internal generic value-parser performance only.
- No dependency, I/O, public API, or output-shape change.
