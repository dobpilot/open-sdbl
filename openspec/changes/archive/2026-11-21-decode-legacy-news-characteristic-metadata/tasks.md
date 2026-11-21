# Tasks

- [x] Retain native CF/XML and reproduce missing metadata with a new exact regression.
- [x] Decode only the measured legacy characteristic profile and select it for projection.
- [x] Verify five ordered attributes, all qualified declarations, owner settings and generated identities against XML, with fail-closed negative cases.
- [x] Run fmt, all-target Clippy, complete unfiltered host workspace tests, warning-free rustdoc and strict validation.
- [x] Return a minimal independently applicable patch against the frozen conversion review base without changing versions or old tests.

## Verification outcome

The new exact native regression was red against the frozen preceding conversion review and passes after implementation. Three new tests verify all five ordered native declarations, owner settings and qualified value types, distinct raw generated object/reference identities and both externally referenced generated type identities; they also reject seven malformed owner variants and eleven malformed membership variants without publishing prefix declarations. The existing characteristic-focused set passes unchanged: eight unit tests including the new three and the previous modern and BUH profiles.

Formatting, all-target Clippy with warnings denied, warning-free rustdoc and strict OpenSpec validation pass. The complete unfiltered workspace run outside the socket-restricted sandbox reports 1027 passed, 1 failed, 13 ignored and zero filtered tests across 84 summary blocks. The only failure is the inherited query_corpus::compiles_the_demo_corpus_as_recorded UUID SQL expectation mismatch; no expectations were changed. Final green remains contingent on that separately pending approval. All package versions and lockfiles remain unchanged at 0.9.20. No release, merge, commit or archive is performed.
