## ADDED Requirements

### Requirement: Acquire catalog default presentation from Config
The library SHALL expose the configured catalog default presentation as AsCode or AsDescription when the acquired owner Config layout and setting are recognized. The setting SHALL NOT be derived from code type or attribute lengths. An unknown setting or layout SHALL remain unknown. Nested custom descriptors SHALL NOT inherit the owner presentation.

#### Scenario: Code presentation
- **WHEN** a recognized catalog owner declares DefaultPresentation AsCode
- **THEN** the acquired descriptor exposes AsCode

#### Scenario: Description presentation
- **WHEN** a recognized catalog owner declares DefaultPresentation AsDescription
- **THEN** the acquired descriptor exposes AsDescription

#### Scenario: Unknown setting
- **WHEN** the acquired catalog default presentation is absent or unrecognized
- **THEN** the descriptor does not invent a presentation from CodeType or lengths
