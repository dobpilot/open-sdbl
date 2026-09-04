## ADDED Requirements

### Requirement: Keep CLI tests aligned with module ownership
Tests for provider, argument, credential, network, output, progress, and
pipeline behavior SHALL live beside the production module that owns that
behavior, while the binary root SHALL retain only orchestration-level tests.

#### Scenario: Locate a CLI behavior test
- **WHEN** a maintainer changes behavior owned by a focused CLI module
- **THEN** its unit tests are discoverable in that module without depending on
  private imports collected by the binary root
