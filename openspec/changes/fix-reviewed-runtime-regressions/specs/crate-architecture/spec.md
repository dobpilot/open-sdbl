## ADDED Requirements

### Requirement: Cancel MSSQL acquisition as a connection-owning operation
MSSQL metadata and configuration acquisition SHALL guard its transaction from
the start of BEGIN through completion of COMMIT or verified rollback. Dropping
an unfinished acquisition future SHALL synchronously mark the session unusable
and drop its connection. Successful completion SHALL permit reuse, and cleanup
SHALL preserve the original acquisition error.

#### Scenario: Caller cancels acquisition
- **WHEN** a caller drops the future during begin, reading, decoding or transaction completion
- **THEN** the session is dead and its connection cannot carry subsequent work

#### Scenario: Acquisition finishes normally
- **WHEN** acquisition commits or a failed acquisition completes verified rollback
- **THEN** the cancellation guard does not discard an otherwise usable connection
