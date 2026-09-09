## ADDED Requirements

### Requirement: Preserve trusted CLI diagnostic layout
Top-level argument and usage diagnostics SHALL render built-in help layout with
real line breaks, while errors containing database-, metadata-, parser-, or
operating-system-derived text SHALL remain escaped before reaching the
terminal. The missing PostgreSQL plaintext opt-in diagnostic SHALL use stable
plain-text fields consisting of an error code, summary, cause, and remediation,
without ANSI styling or the complete command manual.

#### Scenario: Missing plaintext opt-in
- **WHEN** PostgreSQL plaintext mode is requested without the explicit
  insecure opt-in flag
- **THEN** the diagnostic identifies a stable machine-readable code and gives a
  human-readable explanation plus exact alternatives to add the opt-in or
  restore verified TLS

#### Scenario: External error text
- **WHEN** a non-usage error contains terminal control characters
- **THEN** those characters are rendered in escaped textual form
