## 1. Streaming acquisition

- [x] 1.1 Add a fixed SELECT-only Config totals query.
- [x] 1.2 Stream Config rows with bounded in-flight `spawn_blocking` decoding
  while preserving source order and errors.
- [x] 1.3 Move DBNames, SchemaStorage, and final resolution CPU work to the
  blocking pool.

## 2. Progress reporting

- [x] 2.1 Implement a rate-limited percentage bar with resource and byte
  counters and phase labels.
- [x] 2.2 Emit progress only to TTY standard error and leave stdout/non-TTY
  output unchanged.
- [x] 2.3 Add progress-format and streaming decode regression coverage.

## 3. Verification

- [x] 3.1 Verify interactive progress and prompt startup through the reported
  SOCKS5 connection.
- [x] 3.2 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, release artifact verification, and strict OpenSpec
  validation.
