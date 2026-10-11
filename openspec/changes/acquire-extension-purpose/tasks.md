# Tasks

- [x] 1. Obtain all three native purpose variants with CFE/XML/SQL evidence and SHA-256 provenance.
- [x] 2. Record the measured source and structural guards in design.
- [x] 3. Add typed metadata and acquisition purpose with common decoding.
- [x] 4. Test all variants, unknown/truncated records and independence of other properties.
- [x] 5. Run workspace fmt, Clippy, tests, rustdoc and strict OpenSpec without changed SQL/error expectations.
- [ ] 6. Commit implementation and separately archive after successful gates.

## Verification

Native corpus: nine CFE/XML/native-info/ConfigCAS variants, all 75 SHA-256 records verified. Read-only current demo and buh roots use supported measured headers. Five native decoder tests and four acquisition tests passed; old SQL/error expectations and existing tests are unchanged.

Final gates: cargo fmt --all -- --check; cargo clippy --workspace --all-targets --locked -- -D warnings; cargo test --workspace --locked --no-fail-fast; RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked; openspec validate --all --strict. All exit 0. Cargo.lock changes only the three workspace package versions.
