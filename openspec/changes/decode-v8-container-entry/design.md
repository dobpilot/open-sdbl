# Design

Move the algorithm from open-bsl crates/bsl-config-db/src/lib.rs (container_entry, utf16_name, document), retaining native directory-name matching and block handling. One caller-supplied limit applies separately to every assembled document, including directory and name metadata; it is not an aggregate allocation promise. Check declared document size before collecting payload. Reject repeated block offsets. The SDK owns the shared pure decoder; the loader delegates and removes its private parser in the integration workspace. Decode only the container layer: stored-value text remains the existing codec and the SDK common-setting decoder distinguishes the measured multipart ValueStorage form from ordinary internal value text. Materialization into BSL remains the host’s responsibility.

Native evidence: own PostgreSQL scalar.settingsdata and storage.settingsdata with $Stream$ and f0000000001 streams under open-bsl docs/research/fixtures/user-settings-native-1015. Fixtures contain only own values. No original user records or password fields are copied.

## Verification

Five decoder tests pass, including byte-for-byte native root/auxiliary streams, known absence, truncation, bounds, cyclic blocks and corrupt directory entries. The integration loader removes its private decoder and delegates to the SDK; all 480 config-db library tests pass, including eight native-container tests. The previously approved extension-hash fixture correction was synchronized into that isolated copy before the full run. Package all-target Clippy and warning-free rustdoc pass. Full SDK package tests are running separately; this draft is not published or archived.

## Common setting text boundary

DecodedCommonSetting distinguishes ordinary internal value text from the contents of a ValueStorage. decode_common_setting reads $Stream$ through the shared container decoder and validates serialized syntax with the existing parser. For the measured e199ca70-93cf-46ce-a54b-6edc88c3a296 ValueStorage root with one #file reference, it resolves the auxiliary document inside the same container and delegates framing/decompression to decode_stored_value_text. It never reads a filesystem path. Missing roots/auxiliary documents and unsupported ValueStorage reference forms are errors. Neither variant materializes BSL objects in the SDK.
