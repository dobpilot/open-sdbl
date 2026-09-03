## ADDED Requirements

### Requirement: Expose one sealed backend abstraction
Query compilation SHALL be reachable through a sealed backend trait
implemented by the PostgreSQL and MSSQL backend values, so applications can
write backend-generic code against `QueryCompiler<B>` and a single generic
prepared-query type, without the crate committing to open extension.

#### Scenario: Backend-generic application code
- **WHEN** an application writes one function generic over the backend
  trait and calls it with the PostgreSQL and the MSSQL backend
- **THEN** both calls compile queries through the same generic API surface

#### Scenario: Sealed extension point
- **WHEN** an external crate attempts to implement the backend trait for
  its own type
- **THEN** the implementation is rejected at compile time

### Requirement: Keep source modules bounded
The query-compilation implementation SHALL be organized into focused
submodules (diagnostics, AST, parser, dialect, metadata resolution, code
generation) rather than one monolithic module, and shared compilation logic
SHALL exist exactly once regardless of how many sources a query joins.

#### Scenario: Single- and multi-source queries share logic
- **WHEN** the same dereference expression is compiled in a single-source
  query and in a JOIN query
- **THEN** both paths execute the same resolution and join-planning code
  and produce consistent SQL
