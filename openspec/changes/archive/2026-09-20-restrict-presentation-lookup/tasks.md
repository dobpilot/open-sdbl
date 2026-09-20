## 1. The lookup

- [x] 1.1 `compile_presentation_lookup` takes compile options and a mode,
  and applies the decision of its target to the table it reads.
- [x] 1.2 `Prepared::compile_presentation_lookup`, carrying the prepared
  mode; the compiler-level entry point unchanged and documented as
  unfiltered.

## 2. The refusal

- [x] 2.1 `Restricted` stops refusing a deferred reference presentation;
  every other refusal stays.

## 3. Tests

- [x] 3.1 A filtered lookup wraps its target; an unrestricted one does
  not, byte for byte as before.
- [x] 3.2 A denied target yields a predicate no row satisfies.
- [x] 3.3 A missing decision in `Restricted` fails with `Restriction`.
- [x] 3.4 A deferred presentation compiles in `Restricted`.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `openspec validate restrict-presentation-lookup --strict`; archive
