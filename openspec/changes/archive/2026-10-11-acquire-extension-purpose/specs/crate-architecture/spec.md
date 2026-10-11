## ADDED Requirements

### Requirement: Acquire a measured extension purpose
Acquired extensions SHALL expose an optional typed ConfigurationExtensionPurpose with Patch, Customization and AddOn variants. Purpose SHALL come only from native-measured extension data through the shared acquisition decoder. It SHALL remain independent of extension name, activation, scope and version. Acquisition SHALL remain read-only and SHALL NOT add per-BSL-session reads or external dependencies.

#### Scenario: Three measured purposes
- **WHEN** acquisition reads complete native-measured Patch, Customization or AddOn data
- **THEN** it exposes the corresponding typed purpose on either supported SQL provider

#### Scenario: Unknown or damaged data
- **WHEN** the purpose source is unavailable, unsupported, unknown or malformed
- **THEN** purpose is None and unrelated reliably acquired fields remain available

#### Scenario: Other properties change
- **WHEN** an extension name, activation, scope or version changes without changing purpose
- **THEN** its acquired purpose remains unchanged
