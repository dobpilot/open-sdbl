# Tasks

## 1. Implementation

- [x] 1.1 `DbEngine::derive_session` binds each conditional separator's condition constant to the shared area (`false`) below the operator's bindings. Verified live on `unf`: a separated-table query returns rows where it previously failed.
- [x] 1.2 The binding runs before the session-parameter reads, so `ПараметрыОграниченияДоступа` no longer logs a separator warning. Verified by the absent warning in the serve log.

## 2. Finish

- [x] 2.1 `cargo clippy -p open-sdbl-pgwire --all-targets -- -D warnings` and `cargo test -p open-sdbl-pgwire` pass.
- [ ] 2.2 `cargo test --workspace` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` pass.
- [ ] 2.3 `openspec validate serve-shared-data-area-by-default --strict` passes.

## Workflow follow-up

- `openspec archive serve-shared-data-area-by-default` after review.
