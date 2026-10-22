## 1. Contract
- [x] 1.1 Inspect existing identities, AST and physical discovery; validate before code.
## 2. Implementation
- [x] 2.1 Add measured lightweight generated-type/section/enum/member/synonym identity projection and native parity tests.
- [x] 2.2 Add bounded direct-query reference discovery including constants and nested expressions, with positional failure tests.
- [x] 2.3 Add async/blocking extension physical metadata discovery without Config/CAS.
- [x] 2.4 Bump version and document exact discovery limits.
## 3. Verification
- [x] 3.1 Run focused and workspace tests, fmt, Clippy, warning-free doc and strict OpenSpec.
- [x] 3.2 Archive and deliver isolated patch against canonical0.9.32.

Verification: workspace functional tests: 1120 passed, 13 ignored, 0 failed. After the final declaration additions, current unit/binary tests: 376 passed; header/query focused tests: 3 + 5 passed, including 5225 native headers and the 4096-operator boundary. Doctests: 15 passed. Formatting, all-target Clippy, warning-free rustdoc and strict OpenSpec passed. No existing expected results were edited. The slow query corpus was not repeated after the identity-only additions and exact shared-helper lift; the existing compiler boundary regression was rerun successfully.
