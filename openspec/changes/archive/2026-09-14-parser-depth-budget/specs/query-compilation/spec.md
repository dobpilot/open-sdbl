## ADDED Requirements

### Requirement: Nesting budget fits a small stack
The parser SHALL report `TooDeep` before a nested expression can exhaust
the stack of a small thread, and the budget SHALL be 64 nesting levels.

#### Scenario: Deeply nested functions
- **WHEN** a query nests a date function far beyond the budget
- **THEN** the compiler reports that the nesting depth exceeds the limit
  of 64 instead of aborting the process
