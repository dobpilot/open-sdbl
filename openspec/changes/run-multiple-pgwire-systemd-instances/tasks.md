## 1. Specification
- [x] Validate this change strictly before implementation and account for the active `configure-pgwire-toml` change.

## 2. Deployment
- [x] Add the template unit with per-instance TOML and optional environment paths, dedicated user, restart policy and existing service hardening; retain the existing unit.
- [x] Document installation, two database configurations on distinct ports, instance names, credential and TLS file access, shared-account limits, lifecycle, logs, bind conflicts, restart and SIGHUP semantics.

## 3. Verification
- [x] Verify the template with `systemd-analyze verify` in a suitable environment and check instance substitution and credential file configuration. Verification used a temporary unit copy with `/usr/bin/true` in place of the uninstalled executable; debug output confirmed both paths expand to `test_instance`.
- [ ] Exercise two concurrent configured gateway processes against distinct test databases, verify each port reaches its intended backend, and verify stopping or failing one leaves the other available; record any unavailable environment prerequisites.
- Two accessible 1C test databases and their credentials are needed for this live check; neither was supplied in this workspace.
- [ ] Verify the existing single-instance unit and documented configuration remain usable.
- The existing unit is unchanged and passes `systemd-analyze verify` with the same executable substitution; CLI configuration tests pass. An installed service connected to a 1C base was not available for an end-to-end check.
- [x] Run workspace formatting, Clippy with warnings denied, tests, rustdoc with warnings denied, and strict OpenSpec validation. The workspace test suite passed with local sockets enabled outside the sandbox.
