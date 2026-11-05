# Serve the shared data area by default

## Why
A base with a conditionally-separated common attribute (its separation
driven by a Config constant, as `unf` has for constant
`cc906f05-…`) refuses every query against a separated table: the compiler
reports `data separator requires condition constant <guid>` because the
gateway never set the condition. Login succeeds but no catalog, document
or register can be read, and session-parameter derivation logs the same
failure. A single-area, self-hosted base — the gateway's target — has that
constant off, so the gateway should serve the shared (empty) data area by
default and let the queries run.

## What Changes
- When it derives a connection's session, the gateway SHALL set every
  conditional separator's condition constant to the shared area (`false`)
  unless the operator already bound that condition, so a base that is not
  split into data areas is fully queryable without extra flags.
- The derivation that reads `ПараметрыОграниченияДоступа` runs with those
  conditions already set, so its separator warning no longer appears.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: "Derive the session of the authenticated user" — the
  gateway binds conditional-separator conditions to the shared area by
  default, below the operator's own bindings.

## Impact
- Code: `crates/open-sdbl-pgwire` session derivation
  (`DbEngine::derive_session`) sets the conditions from
  `MetadataSnapshot::separators()`; no core change, no new dependency, no
  database query added.
- Serving a specific data area of a multi-area (1cFresh) base is out of
  scope; such a base would need the operator to bind the separator value,
  a later change.
