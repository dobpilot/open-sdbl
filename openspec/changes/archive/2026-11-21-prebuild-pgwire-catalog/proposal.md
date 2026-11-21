# Prebuild the complete pgwire catalog

## Why
The first catalog request of every connection currently compiles every relation twice and materializes the complete virtual catalog on the connection task. Large configurations therefore pay a linear cold-request cost even in unrestricted mode.

## What Changes
- Build the complete supported catalog concurrently while acquiring gateway state, before clients are accepted.
- Share the immutable catalog and its relation-access metadata between connections.
- Return the shared catalog after cheap session personalization in unrestricted mode.
- Filter the prebuilt relations, columns, schemas, types and comments by the authenticated user's rights in restricted mode without recompiling relations.
- Rebuild the complete catalog on metadata refresh and preserve stable OIDs and existing visibility rules.

## Capabilities

### Modified Capabilities
- `pgwire-catalog`: initialization-time parallel construction and request-time sharing or authorization filtering.
- `pgwire-gateway`: state acquisition and refresh include the complete prebuilt catalog.

## Impact
- Gateway state, catalog model, connection catalog selection, concurrency, tests and documentation.
