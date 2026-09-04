## ADDED Requirements

### Requirement: Preserve trusted CLI diagnostic layout
Top-level argument and usage diagnostics SHALL render built-in help layout with
real line breaks, while errors containing database-, metadata-, parser-, or
operating-system-derived text SHALL remain escaped before reaching the
terminal.

#### Scenario: Missing plaintext opt-in
- **WHEN** PostgreSQL plaintext mode is requested without the explicit
  insecure opt-in flag
- **THEN** the diagnostic and following help are separated by real line breaks
  and do not display escaped `\n` text

#### Scenario: External error text
- **WHEN** a non-usage error contains terminal control characters
- **THEN** those characters are rendered in escaped textual form
