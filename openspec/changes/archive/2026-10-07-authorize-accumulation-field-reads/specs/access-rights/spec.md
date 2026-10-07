## ADDED Requirements

### Requirement: Map verified accumulation register programmatic reads
For a recognized native accumulation register owner with verified Balance register kind and complete own dimension/resource collections, the SDK SHALL authorize reads of those own GUID fields with nonempty recognized declarations through the selected roles' object Read grants. This rule SHALL NOT depend on UI View or attribute-default flags. Missing users, unread roles, unresolved membership, ambiguous member paths and unsupported standard fields SHALL remain fail closed. Supported explicit member Read refusals SHALL retain their conservative refusal. Field checks SHALL NOT bypass existing RLS decisions or query compilation.

#### Scenario: Owner Read grants dimension and resource
- **WHEN** a selected role grants owner Read for verified dimension/resource fields with View false and either attribute-default flag
- **THEN** the complete field check succeeds

#### Scenario: Owner Read is absent
- **WHEN** no selected role grants owner Read
- **THEN** reading either field fails before SQL, including an ALLOWED query

#### Scenario: Native membership is unknown
- **WHEN** a field is not proven by complete native dimension/resource collections and a recognized declaration
- **THEN** the native mapping does not grant it

#### Scenario: RLS remains applicable
- **WHEN** verified fields pass their field checks and selected roles restrict owner Read
- **THEN** existing access decisions preserve the restrictions and combine multiple granting roles with OR
