## 1. Contract
- [x] 1.1 Inspect available decoders, identities and physical inputs, and validate this change before code.
## 2. Implementation
- [x] 2.1 Add reusable physical-input acquisition and bounded filename-only discovery on both session forms/providers.
- [x] 2.2 Add measured own-header projection without detailed descriptor construction.
- [x] 2.3 Add explicit-resource resolution with physical projection preserving separators and GUID identities.
- [x] 2.4 Add focused tests for no Config reads, header parity, subset queries, extension inputs and role-only AccessStore reuse.
- [x] 2.5 Bump packages to 0.9.32 and document reuse and fingerprint limits.
## 3. Verification
- [x] 3.1 Run workspace fmt, Clippy, tests, warning-free rustdoc and strict OpenSpec.
- [x] 3.2 Archive this SDK change and deliver a clean patch against supplied 0.9.31.

Verification: focused core 5/5 and DB 5/5; workspace functional suite and doc retry total 1127 passed, 13 ignored. The first shared-target doctest attempt hit E0460 while another checkout rebuilt SDK artifacts; isolated-in-time doctest retry passed all 15. No existing test expectations changed.
