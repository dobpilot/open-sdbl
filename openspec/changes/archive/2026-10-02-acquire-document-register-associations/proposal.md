# Change: Acquire document register associations

## Why
Hosts need complete RegisterRecords identities without deriving them from SQL or incidental GUIDs.

## What Changes
- Add `ConfigDescriptor::document_registers: Option<Vec<Guid>>`.
- Decode the measured document-owned counted reference collection in configuration order.

## Impact
- Affected spec: onec-metadata.
- Additive metadata API, version remains 0.9.6; no SQL authorization changes.
