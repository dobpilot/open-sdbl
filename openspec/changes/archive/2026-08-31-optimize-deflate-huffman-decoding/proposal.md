## Why

Large 1C Config resources make CLI startup appear hung even after the database
connection succeeds. Profiling the reported information base attributes over
77% of CPU samples to Huffman symbol decoding because every decoded symbol
linearly scans every tree entry.

## What Changes

- Replace per-symbol Huffman entry scans with a bounded direct lookup table.
- Read small bit fields from a byte window instead of advancing one bit at a
  time.
- Preserve raw-DEFLATE validation, output limits, diagnostics, and decoded
  bytes.
- Verify the optimization against stored, fixed, dynamic, truncated, invalid,
  and oversized streams and the reported live metadata startup.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: make bounded raw-DEFLATE decoding scale with decoded symbols
  rather than symbols multiplied by tree entries.

## Impact

- Internal core metadata-decoder implementation and performance.
- No production dependency, I/O, public API, or accepted-format change.
