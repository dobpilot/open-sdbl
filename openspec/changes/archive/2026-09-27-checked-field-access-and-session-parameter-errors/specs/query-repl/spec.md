## ADDED Requirements

### Requirement: Expose a missing query parameter name structurally
A query diagnostic caused by an unbound named parameter SHALL expose the name without requiring parsing of its message. A data-separator failure SHALL additionally identify that the missing value is a session parameter. Other parameter diagnostics SHALL NOT claim to be missing-session-parameter failures.

#### Scenario: An unbound named parameter
- **WHEN** compilation reaches `&Параметр` with no binding
- **THEN** the diagnostic exposes `Параметр` as its missing name while retaining its source position and message

#### Scenario: A missing separator parameter
- **WHEN** a data separator requires an unset session parameter
- **THEN** the diagnostic exposes its name and identifies the missing value as a session parameter

#### Scenario: An unused parameter
- **WHEN** a supplied parameter is never referenced
- **THEN** its diagnostic has no missing-parameter name
