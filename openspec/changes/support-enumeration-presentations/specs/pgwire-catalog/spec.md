## ADDED Requirements

### Requirement: Describe enumeration columns as text
The virtual catalog SHALL describe columns with one statically known
enumeration target using the same PostgreSQL text type that the gateway emits
for direct query results.

#### Scenario: Discover an enumeration field
- **WHEN** a client inspects an enumeration-typed field through the virtual catalog
- **THEN** its reported PostgreSQL type is text
