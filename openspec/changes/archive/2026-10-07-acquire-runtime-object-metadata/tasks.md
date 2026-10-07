## 1. Evidence and implementation
- [x] Record native XML/Config controls for business-process numbering and own members, including sections and filling policies.
- [x] Record native accumulation-register kinds, empty collections and ordered members.
- [x] Add documented APIs and measured projection with complete membership validation.
- [x] Test corruption, order, isolation, snapshots and selective acquisition.
- [x] Check read-only demo acquisition without running original BSL.
- [x] Set all package and lock versions to 0.9.9.

## 2. Validation
- [x] cargo fmt --all -- --check
- [x] cargo clippy --workspace --all-targets --locked -- -D warnings
- [x] cargo test --workspace --locked
- [x] RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
- [x] openspec validate --all --strict
- [x] Archive separately after green checks.
