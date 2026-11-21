## ADDED Requirements

### Requirement: Evaluate DBeaver column introspection within budgets
The catalog SHALL evaluate DBeaver's attribute query for one relation in a populated catalog without exhausting existing allocation limits. Optimizations SHALL preserve filtering, outer-join, ordering, and column projection semantics.

#### Scenario: Column listing in a populated configuration
- **WHEN** DBeaver selects `a.*` from `pg_attribute` joined to `pg_class` and filters by one relation OID
- **THEN** the gateway returns that relation's visible columns within the existing budgets
