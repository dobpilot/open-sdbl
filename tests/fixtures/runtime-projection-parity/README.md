# Runtime projection baseline

`before.tsv` was captured before the runtime module refactor from the decoder
at commit `78c13b7`. It covers all 461 existing `.deflate` fixtures, without
modifying their bytes or existing expected results.

Each row records the relative path, expected resource GUID, and FNV-1a checksums
of the complete `Debug` result from `parse_config_resource_bounded`: original
input, damaged final collection, and truncated compressed input. The result
includes every descriptor field, ordering, None versus known empty, decoded
size and error diagnostics. The resource GUID is the first descriptor identity
found in a preliminary decode, or a fixed non-nil sentinel if none exists.

`tests/runtime_projection_parity.rs` checks these frozen records. It has no
update mode. A deliberate behavior or Debug-format change needs explicit review
of affected cases; do not regenerate the baseline to accommodate a refactor.
Existing native XML assertions and targeted malformed-input tests remain the
readable semantic checks alongside these whole-result fingerprints.
