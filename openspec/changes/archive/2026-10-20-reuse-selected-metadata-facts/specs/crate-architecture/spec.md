## ADDED Requirements

### Requirement: Reuse admitted immutable selected resources
A public cache SHALL admit newly reached immutable Config resources under ordinary count/compressed/decoded byte limits, parse each admitted resource once, and retain parsed facts without retaining compressed buffers. Duplicate filenames SHALL be rejected before decoding. Failed admission SHALL preserve existing facts and budgets.

#### Scenario: Resolve after later reachability
- **WHEN** a caller admits a new resource after an earlier successful admission
- **THEN** subsequent resolution uses both resources' parsed facts without decoding the earlier resource or retaining its compressed buffer

#### Scenario: Admission failure
- **WHEN** an admission is duplicate, malformed or over budget
- **THEN** it fails without altering the previously admitted facts or consuming their remaining budget

### Requirement: Preserve strict selected resolution
Cached resolution SHALL use the same finalization, explicit extension handling, strict separator/missing descriptor checks, ordering, stable identities, reports and fingerprints as stateless selected resolution. The existing stateless function SHALL preserve its contract. Prepared-query snapshot validation SHALL remain enforced.

#### Scenario: Native facts parity
- **WHEN** identical native selected resources including separator and extension cases are resolved through either path
- **THEN** snapshot facts, report, fingerprint and prepared-query compatibility agree

### Requirement: Index repeated pure-resolution name lookups equivalently
Per-resolution schema and dependency-owner indexes SHALL preserve the existing identifier equality and source-order precedence. They SHALL NOT change names_equal, serialized formats or metadata interpretation.

#### Scenario: Competing service-table matches
- **WHEN** alias, numbered and extra-dimension candidates match different schema rows
- **THEN** indexed lookup selects the same earliest matching row as the original scan

#### Scenario: Competing dependency owners
- **WHEN** multiple objects share an equivalent physical table name
- **THEN** dependency resolution selects the same first eligible nonservice owner and preserves the original duplicate-table exclusion
