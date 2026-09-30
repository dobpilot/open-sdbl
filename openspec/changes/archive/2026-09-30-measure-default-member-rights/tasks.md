## 1. Evidence
- [x] 1.1 Record native actual query/manager reads for default-attribute flags true and false.
- [x] 1.2 Record an explicit constant object Read refusal and preserve the diagnostic boundary for custom/standard members.

## 2. Implementation
- [x] 2.1 Authorize verified constant values through object Read, with grant, denial, cross-role, unread-role and mismatched-ID regressions.
- [x] 2.2 Set all workspace package versions and lock entries to 0.9.5.

## 3. Validation
- [x] 3.1 Run formatting, Clippy, workspace tests, rustdoc and strict OpenSpec validation.
- [x] 3.2 Verify live PostgreSQL authorization on UNF without writing data.
- [x] 3.3 Archive the change after successful verification.
