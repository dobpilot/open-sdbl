## ADDED Requirements

### Requirement: Acquire functional-option fields with populated Content
The SDK SHALL acquire measured location and privileged retrieval fields for functional options with validated populated Content. It SHALL reject truncated, unsupported, invalid-count, invalid-reference and invalid-flag forms without publishing guessed fields.

#### Scenario: Native populated options
- **WHEN** native XML and Config declare two Content references and privileged mode false or true
- **THEN** the descriptor retains the validated location and exact flag independently of Content emptiness

#### Scenario: Malformed Content or flag
- **WHEN** a counted Content reference or retrieval flag is invalid
- **THEN** acquisition leaves option fields unknown
