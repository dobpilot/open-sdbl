# Tasks

## 1. Core

- [x] 1.1 `decode_stored_password` reads `H1,H2` at the measured offset; `decode_user_data` keeps it in a private `UserData::stored_password`. Verified by a synthetic-blob decode test.
- [x] 1.2 Manual `UserData` `Debug` omits the stored value; the field is private. Verified by a `Debug`-redaction test.
- [x] 1.3 `UserData::verify_password` verifies against the stored value under the confirmed source, else returns `INDETERMINATE`. Verified by match/no-match/absent unit tests.

## 2. Gateway

- [x] 2.1 `authenticate` reports a confirmed source for `UserData` evidence so a `MATCH` permits login. Verified by an end-to-end login test through the `UserData` path.
- [x] 2.2 CLI startup line no longer claims every login fails closed. Verified by reading `serve` output.

## 3. Finish

- [x] 3.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` all pass.
- [x] 3.2 README (Russian) describes confirmed live login and the confidence limits of extended formats (updated in section 4).
- [x] 3.3 `openspec validate authenticate-stored-password-from-v8users --strict` passes.

## Workflow follow-up

- `openspec archive authenticate-stored-password-from-v8users` after review.

## 4. Native extended record regression (2026-10-27)

- [x] 4.1 Decode the measured extended suffix using its optional-field count; retain legacy and packed forms and fail closed for malformed extended records.
- [x] 4.2 Add synthetic SHA-256/SHA-512 record tests through UserData and the gateway, with malformed/wrong-password/redaction controls.
- [x] 4.3 Verify the authorized current unf user through a read-only live test and a temporary gateway, without storing or printing credential material.
- [x] 4.4 Run formatting, Clippy, tests, rustdoc, strict change validation and release build; update the measurement and limits documentation. Workspace tests and all-feature gateway tests pass. Repository-wide strict validation retains the 14 unrelated pre-existing failures; this change validates independently.
