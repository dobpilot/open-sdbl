## Tasks
- [x] 1. Retain native Config/XML/EDT provenance and add a failing native membership regression.
- [x] 2. Add documented optional membership and project measured owner/attribute shape; bump 0.9.43.
- [x] 3. Test malformed owner/collection, count, duplicate and missing typed member, unknown wrapper, and verified empty.
- [x] 4. Run formatting, workspace Clippy, tests, rustdoc and strict OpenSpec checks.
- [x] 5. Merge specification delta and archive after successful checks.

The four focused native/malformed/empty/type-resolution tests passed. Complete integrated test coverage is provided by the workspace attempt (all core/CLI groups passed), an unchanged full DB package replay outside the socket-restricting sandbox, and workspace doctests. Formatting, workspace Clippy, rustdoc with warnings denied and strict OpenSpec checks passed. Existing tests only gained the new required None field in five structure literals; assertions and expectations are unchanged.
