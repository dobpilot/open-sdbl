## 1. Streaming DBNames projection

- [x] 1.1 Implement a validating brace cursor that retains at most three scalar
  candidates per nesting level.
- [x] 1.2 Project compatible DBNames entries directly in source order and keep
  public generic parsing unchanged.
- [x] 1.3 Add parity and malformed-input regression coverage.

## 2. Verification

- [x] 2.1 Rebuild release and verify the reported live console reaches its
  prompt through SOCKS5.
- [x] 2.2 Compare live timing/profile with the generic-tree baseline.
- [x] 2.3 Run all repository quality gates and strict OpenSpec validation.
