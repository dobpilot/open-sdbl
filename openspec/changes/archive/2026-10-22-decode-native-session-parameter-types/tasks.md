# Tasks

- [x] 1. Validate strictly before code and reproduce native UUID loss.
- [x] 2. Project only the measured complete root typed shape; set version 0.9.42.
- [x] 3. Test native paired evidence, unknown/incomplete declarations and malformed envelopes without changing existing tests.
- [x] 4. Run focused checks and coordinated complete gates, then archive.


The 0.9.42 focused regression passed (4/4). SDK 0.9.43 integrated gates include all four tests: formatting, workspace Clippy, complete core/CLI tests, full DB package replay outside the socket-restricting sandbox, workspace doctests, rustdoc with warnings denied, and strict OpenSpec validation passed. The initial workspace attempt's eight DB socket failures were environmental EPERM; the entire DB package was rerun unchanged with loopback available and passed.
