# Describe deferred presentations as text

## Why
The gateway advertises deferred `ПредставлениеСсылки` as `bytea` even after replacing the internal reference payload with a Unicode string. JDBC clients interpret the UTF-8 bytes as binary data.

## What Changes
- Describe and encode deferred presentation columns as PostgreSQL text in simple and extended protocol.
- Preserve binary typing for actual reference columns and retain the internal reference payload for lookup.

## Capabilities

### Modified Capabilities
- `pgwire-gateway`: expose resolved deferred presentations as text.

## Impact
- PostgreSQL wire result descriptions and encoding, tests and client compatibility.
