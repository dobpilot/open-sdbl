## MODIFIED Requirements

### Requirement: Emit PostgreSQL SQL for a stated minimum server
Generated PostgreSQL SQL SHALL target PostgreSQL 13 or newer — the
releases the vendor ships with the platform — so that one stateless
backend serves every supported server. Constructs newer than that
minimum SHALL NOT be generated.

#### Scenario: Balance anchor aggregate
- **WHEN** an accumulation-register balance query is compiled
- **THEN** the anchor period uses `MAX(CASE WHEN … END)` rather than
  `FILTER (WHERE …)`

#### Scenario: Full join of two sources
- **WHEN** a branch joins two sources with `ПОЛНОЕ СОЕДИНЕНИЕ`
- **THEN** generated SQL contains a native `FULL JOIN`, which every
  targeted server plans directly
