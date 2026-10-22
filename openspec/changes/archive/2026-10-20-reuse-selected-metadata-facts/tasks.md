## 1. Contract
- [x] 1.1 Inspect existing resolver/cache paths and validate this API before implementation.
## 2. Implementation
- [x] 2.1 Add immutable parsed-fact admission with ordinary limits and no compressed retention.
- [x] 2.2 Share strict finalization/resolution while preserving the stateless contract.
- [x] 2.3 Add equivalent service-table and dependency-owner indexes with first-match tests.
- [x] 2.4 Add native cached/stateless parity, immutable-admission and budget regression tests.
- [x] 2.5 Bump version and document reuse, ownership and fingerprint limits.
## 3. Verification
- [x] 3.1 Run focused/workspace tests, fmt, Clippy, warning-free docs and strict OpenSpec.
- [x] 3.2 Archive and deliver early API patch plus final delta against canonical0.9.33.

Verification: focused cache 5/5 and index 2/2; workspace 1144 passed, 13 ignored; fmt, Clippy all targets with denied warnings, warning-free docs, and strict validation passed. No existing expectations changed.
