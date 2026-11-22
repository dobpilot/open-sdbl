# Decode own version 1 extension object headers

## Why

The SDK decodes the identity and name of an extension's own objects with version 1 headers, but leaves their belonging unknown. Consumers that need to distinguish own objects from adopted objects cannot load these extensions.

## What Changes

Decode native-confirmed version 1 own-object headers with a valid property-pair tail as `Own` without an adoption target. Retain the adopted result for valid version 1 adopted headers and the unknown result for unsupported tails.

## Capabilities

### Modified Capabilities

- `onec-metadata` documents the version 1 header and its fields.
- `crate-architecture` covers acquisition of measured legacy extension object belonging.

## Impact

The shared object header decoder changes. Database acquisition uses the same decoder and completeness rules.
