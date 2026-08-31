## Why

After removing the DEFLATE Huffman bottleneck, live startup profiling shows the
next dominant cost in building, traversing, and dropping a complete generic
`Value` tree for a very large DBNames resource. Only three-scalar DBNames entries
are needed from that tree.

## What Changes

- Parse DBNames brace serialization with a validating streaming projection.
- Retain only valid `{GUID,"Alias",Number}` records while recursively validating
  the complete input.
- Avoid allocating generic list nodes and irrelevant scalar strings for
  DBNames.
- Preserve the public generic `parse_serialized` API for Config,
  SchemaStorage, and callers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: scale authoritative DBNames projection without constructing
  an unnecessary full value tree.

## Impact

- Internal DBNames parsing performance and peak memory.
- Accepted syntax, source-order entries, diagnostics, and public APIs remain
  compatible.
