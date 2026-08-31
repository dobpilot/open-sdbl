## ADDED Requirements

### Requirement: Report metadata-loading progress without contaminating output
During PostgreSQL metadata acquisition, the CLI SHALL show a rate-limited
progress bar with phase, completed/total Config resources, compressed bytes,
percentage, and elapsed completion summary only when standard error is a
terminal. Progress SHALL be written to standard error. Standard output and
non-TTY standard error SHALL contain no progress rendering or terminal control
sequences.

#### Scenario: Interactive metadata loading
- **WHEN** the CLI acquires metadata with standard error attached to a terminal
- **THEN** the user sees phase changes and Config completion advance to 100%

#### Scenario: Redirected metadata snapshot
- **WHEN** `open-sdbl metadata postgres` stdout is redirected or piped
- **THEN** stdout contains only the existing tabular snapshot records

#### Scenario: Noninteractive diagnostics
- **WHEN** standard error is not a terminal
- **THEN** progress rendering is suppressed while ordinary errors remain
  available on standard error
