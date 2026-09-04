## 1. CLI test ownership

- [ ] 1.1 Inventory the remaining tests in `crates/open-sdbl-cli/src/main.rs`
  and move each provider, argument, credential, network, output, progress, and
  pipeline test beside its owning module.
- [ ] 1.2 Keep only orchestration tests in `main.rs` and remove coverage that
  duplicates module-local or integration tests.

## 2. Representative fuzzing

- [ ] 2.1 Extend the fixed query fuzz snapshot so valid reference
  dereferences and tabular-section sources are reachable.
- [ ] 2.2 Add a bounded CI build/check of `fuzz/Cargo.toml`; do not run an
  unbounded fuzz campaign in regular CI.

## 3. Verification

- [ ] 3.1 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, and strict OpenSpec validation.
