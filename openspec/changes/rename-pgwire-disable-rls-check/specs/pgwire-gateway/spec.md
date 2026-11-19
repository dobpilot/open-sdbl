## ADDED Requirements

### Requirement: Name the access override consistently
The CLI SHALL accept the explicit flag `--disable-rls-check` and the TOML
boolean field `disable_rls_check` to enable the existing unrestricted gateway
mode. Both SHALL default to disabled. The former names `--unrestricted-access`
and `unrestricted_access` SHALL be rejected. Help and documentation SHALL
explain that the override bypasses role, object, field and row restrictions,
while authentication remains required.

#### Scenario: Enable the override
- **WHEN** an operator passes `--disable-rls-check` or sets
  `disable_rls_check = true` in gateway TOML
- **THEN** the gateway uses its existing unrestricted access behavior

#### Scenario: Leave checks enabled
- **WHEN** the flag is absent or the TOML field is false
- **THEN** normal role, field and row checks remain enabled

#### Scenario: Reject old and invalid names
- **WHEN** an operator supplies the old CLI flag or old TOML field, or a
  non-boolean `disable_rls_check` value
- **THEN** the command fails with a usage error

#### Scenario: Reuse configuration for another command
- **WHEN** `console`, `repl` or `metadata` reads a valid TOML file containing
  `disable_rls_check`
- **THEN** it validates the field's boolean type and ignores its value
