## 1. Local Config ordering

- [x] 1.1 Remove `ORDER BY filename` from the fixed Config query.
- [x] 1.2 Retain decoded resource groups and sort them locally by filename.
- [x] 1.3 Add regression coverage for database-order-independent output.

## 2. Verification

- [x] 2.1 Compare server plans and execution times with and without ordering.
- [x] 2.2 A/B test release startup through the reported SOCKS5 connection.
- [x] 2.3 Run repository quality gates and strict OpenSpec validation.
