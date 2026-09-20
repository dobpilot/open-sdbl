## 1. Resolution

- [x] 1.1 `MetadataObject` and `MetadataField` carry `synonyms` and
  `comment` from the descriptor they resolve from.
- [x] 1.2 `synonym(language)` and `presentation(language)` on both, with
  the trim-and-empty rule; `object_synonym` and `object_presentation` on
  the snapshot.

## 2. The console

- [x] 2.1 The document presentation uses the accessor instead of scanning
  `descriptors()`; its output is unchanged.

## 3. Tests

- [x] 3.1 An attribute synonym reaches the resolved field, and the name
  stays the metadata name.
- [x] 3.2 A missing language, a blank synonym, and an item with no
  descriptor all fall back to the name.
- [x] 3.3 A descriptor comment reaches the resolved item.
- [x] 3.4 The console's presentation output is unchanged.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `openspec validate --all --strict`
- [x] 4.6 README and `docs/` mention the accessors; archive the change
