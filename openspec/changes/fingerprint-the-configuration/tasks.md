## 1. The statements

- [x] 1.1 `PostgresMetadataQueries::CONFIG_FINGERPRINT` and the SQL Server
  counterpart, matching the same resources as the acquisition.
- [x] 1.2 Both added to `all()`, which the coverage tests read.

## 2. Visibility

- [x] 2.1 `SnapshotFingerprint` and `MetadataSnapshot::fingerprint` public
  and documented, with what each fingerprint does and does not answer.

## 3. Tests

- [x] 3.1 Both statements are SELECT-only, name no `PartNo`, and match the
  GUID-shaped resource names the acquisition matches.
- [x] 3.2 The coverage lists include them.
- [x] 3.3 A live SQL Server test: the value is stable across two reads of
  an unchanged base, and the digest expression separates two payloads of
  equal length — checked against literals, so no base is written to.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `cargo tree -p open-sdbl -e normal` still empty
- [x] 4.6 `openspec validate fingerprint-the-configuration --strict`; archive
