# Read a 1C object by navigation link

## Why

Gateway clients can query a 1C table, but opening a single object by its
navigation link currently requires constructing a query and decoding its
reference.

## What Changes

- Add the pgwire function `onec_get_object(link)` for reading an object
  in the gateway's information base.
- Return a JSON object with the standard and custom attributes of the main
  row. The PostgreSQL result column has type `jsonb`.
- Apply the usual gateway authorization and data separation rules for
  reading object data.

## Capabilities

### Modified Capabilities

- `pgwire-gateway`: object lookup by 1C navigation link.

## Impact

Pgwire dispatch and result encoding. Lookup uses the existing SDBL compiler
and read-only database session for both backing providers.
