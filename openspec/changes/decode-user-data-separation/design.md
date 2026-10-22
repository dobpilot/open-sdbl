# Design

Reuse the existing XOR decoder and serialized-value parser. Native own PostgreSQL controls from 2026-10-15 establish a 38-field empty record and a 39-field record with one string binding: field 17 is the count, followed by count two-string lists and a 20-field suffix. Recognize only this layout with the measured roles position and GUID sentinels; unsupported values, duplicate names and inconsistent counts make the whole list unknown. Some(empty) is distinct from None. No partial list or password-bearing raw record is exposed.

Evidence: open-bsl docs/plans/open-sdbl-user-data-separation-request.md and private native artifacts /tmp/open-bsl-user-settings-native-1015. Tests use sanitized constructed records; these are parser tests, not replacement platform evidence. This draft does not bump or publish the SDK version.

## Verification

Both private native user blobs decode to the exact expected complete lists (empty and one string binding), without printing raw user records. Five focused parser tests and all 109 library tests pass. Package all-target Clippy, formatting and rustdoc with warnings denied pass. Workspace integration and the open-bsl principal adapter remain separate pending work; this change is not archived or published.

## Historical record control

Read-only UNF acquisition found base lengths 31, 32 and 37. Dedicated controls reconstruct those layouts from the two own native user records, retaining a consistent counted optional field at position 30 after bindings. Each control is read by the native users manager, independently checking empty count zero and nonempty count one with the exact string value. These controls establish native reader acceptance, not production provenance of the reconstructed bytes. The first reconstruction removed a counted field without changing its count and caused a native client failure; it is retained as failed evidence and is not a positive result.

Support is limited to the verified base lengths 31/32/37/38. The optional field count is zero or one and agrees with the record length; its payload, when present, is a string. The six-field extended suffix is validated by its measured field kinds. Any inconsistent suffix remains unknown.
