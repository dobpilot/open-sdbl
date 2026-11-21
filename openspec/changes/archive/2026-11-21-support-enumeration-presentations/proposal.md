# Present enumeration values

## Why
The default reference presentation policy treats an enumeration like a normal
object with a display field. Its values instead have names and synonyms in
configuration metadata, so displaying the reference field currently produces
binary data rather than the value's readable presentation.

## What Changes
- Produce enumeration value presentations from their metadata identities and
  Russian synonyms, falling back to the declared name.
- Support both presentation functions for statically typed and deferred enum
  references, including source-free enumeration constants, through the shared
  presentation policy on PostgreSQL and MSSQL.
- Preserve authentication, authorization, separator handling, existing
  reference joins and behavior for non-enumeration references.
- Return directly selected, statically typed enumeration references as their
  text presentations through pgwire instead of `bytea`; SQL evaluation,
  UUID and type functions continue to use the reference value.

## Capabilities

### Modified Capabilities
- `query-compilation`: a structured, validated enumeration presentation plan.
- `query-repl`: default enumeration presentation in the console and gateway.
- `pgwire-gateway`: enumeration result columns are text instead of bytea.
- `pgwire-catalog`: enumeration columns are described consistently as text.

## Impact
Core structured presentation plan and SQL generation (the new public enum
variant affects exhaustive downstream matches); shared application presentation
policy and pgwire wire types. Clients of enumeration result columns must adapt
from `bytea` to `text`. No new production dependencies or host I/O in the core.
