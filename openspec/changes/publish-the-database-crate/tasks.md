## 1. The manifest

- [x] 1.1 Drop `publish = false` from `crates/open-sdbl-db/Cargo.toml`.
- [x] 1.2 Give its `open-sdbl` dependency the workspace version beside the
  path.

## 2. Checks

- [x] 2.1 The manifest declares no `publish = false` and its `open-sdbl`
  dependency names a version; `cargo package -p open-sdbl-db --no-verify`
  gets past the flag and stops only on `open-sdbl` not being in the registry
- [x] 2.2 `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`,
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`,
  `cargo build --release --locked`
- [x] 2.3 `openspec validate publish-the-database-crate --strict`, then archive
