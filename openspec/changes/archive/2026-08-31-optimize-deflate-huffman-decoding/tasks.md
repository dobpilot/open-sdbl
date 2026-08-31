## 1. Huffman lookup optimization

- [x] 1.1 Expand validated canonical codes into a bounded direct lookup table.
- [x] 1.2 Add byte-window bit peeking and advance by the selected code's actual
  length.
- [x] 1.3 Preserve malformed-stream diagnostics and add edge-case regression
  coverage.

## 2. Verification

- [x] 2.1 Rebuild release and verify the reported live console reaches its
  prompt through SOCKS5.
- [x] 2.2 Compare the optimized live profile/timing with the baseline profile.
- [x] 2.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
