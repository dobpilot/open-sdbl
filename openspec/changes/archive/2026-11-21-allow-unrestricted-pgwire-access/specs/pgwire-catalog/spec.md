## ADDED Requirements

### Requirement: Unrestricted catalog access by operator choice
When explicitly configured for unrestricted access, the gateway SHALL expose all supported metadata relations and fields after successful authentication without checking role rights. The default SHALL continue to enforce rights.

#### Scenario: Unrestricted catalog
- **WHEN** an authenticated user has no role read rights and unrestricted access is enabled
- **THEN** supported relations and fields are visible in the virtual catalog
