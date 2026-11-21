## Tasks
- [x] Acquire native XML from a byte-preserving complete CF in an own infobase.
- [x] Add a failing full-membership test against all 27 native members.
- [x] Accept only the measured homogeneous legacy wrapper in the existing decoder.
- [x] Verify malformed/mixed/duplicate/unknown-type cases remain unknown.
- [x] Replay original BUH and record the next terminal result.
- [ ] Run workspace formatting, Clippy, tests, rustdoc and strict validation.

Remaining gate: update the separately proposed UUID corpus SQL expectations only after user approval, then run the unfiltered workspace suite. Current remaining suite: 987 passed, 0 failed, 13 ignored, one corpus test filtered; formatting, Clippy, rustdoc and strict validation pass.
