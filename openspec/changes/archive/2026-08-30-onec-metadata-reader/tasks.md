## 1. Resource decoding

- [x] 1.1 Implement a bounded raw-DEFLATE decoder for stored, fixed, and dynamic blocks and verify all block types plus malformed input with unit tests.
- [x] 1.2 Implement the BOM-aware nested brace-value parser and verify atoms, nested lists, doubled quotes, and positional failures with unit tests.

## 2. Authoritative metadata projections

- [x] 2.1 Implement GUID validation and DBNames parsing/indexing, and verify aliases, duplicate GUID entries, field lookup, separators, and fatal empty input with unit tests.
- [x] 2.2 Implement bare-GUID Config descriptor parsing and verify names, localized synonyms, marker capture, and rejection of suffixed resource names with unit tests.
- [x] 2.3 Implement SchemaStorage table, column, reference, and index projection and verify it against representative 8.3.27 schema fragments.

## 3. Resolution and PostgreSQL normalization

- [x] 3.1 Implement tabular-kind and canonical-prefix resolution and verify all specified aliases, including the ambiguous AccRg/Acc and Chrc/CKinds/CRg families.
- [x] 3.2 Implement physical-to-logical compound-column collapsing and separator removal and verify reference, enumeration, storage, and compound groups with unit tests.
- [x] 3.3 Implement longest-token-first PostgreSQL recasing and live-catalog comparison and verify table, field, suffix, and index-key cases with unit tests.

## 4. Read-only PostgreSQL CLI

- [x] 4.1 Add `open-sdbl metadata postgres` option parsing and fixed SELECT-only psql acquisition, and verify help, argument validation, environment handling, and nonzero psql failures with CLI tests.
- [x] 4.2 Render the stable tab-separated resolution report and verify resolved, unnamed, and missing-live-table rows with CLI tests.
- [x] 4.3 Run the command against PostgreSQL `192.168.166.15/test` in an enforced read-only session and independently compare decoded DBNames and SchemaStorage evidence.

## 5. 1C conformance fixture

- [x] 5.1 Use the Unica workflow to create a uniquely named catalog and attribute in the disposable `test` information base and verify their logical GUIDs and physical numbers through Config, DBNames, SchemaStorage, and PostgreSQL catalogs.
- [x] 5.2 Capture minimal non-secret serialized fixtures from the verified objects and add regression tests proving end-to-end logical-to-physical resolution.

## 6. Verification

- [x] 6.1 Run `cargo fmt --all -- --check` and verify no formatting differences.
- [x] 6.2 Run `cargo clippy --all-targets -- -D warnings` and verify no diagnostics.
- [x] 6.3 Run `cargo test` and verify all unit, CLI, and conformance tests pass.
- [x] 6.4 Run `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` and verify documentation succeeds.
- [x] 6.5 Run `openspec validate onec-metadata-reader --strict` and verify the completed change remains valid.
