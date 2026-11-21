# Inspect configuration metadata through pgwire

## Why

Clients need to locate configuration objects and inspect their textual source
without exporting the configuration through Designer.

## What Changes

- Add `onec_find_metadata` with an optional substring filter and
  `onec_show_metadata` for textual BSL and XML.
- Allow every authenticated gateway user to use these functions, independently
  of the user's 1C data rights and the gateway's RLS setting.
- Discover nested forms, modules, and layouts using their logical 1C paths.
- Report unavailable or non-text resources without representing binary as text.

## Capabilities

### Modified Capabilities

- `pgwire-gateway`: authenticated metadata discovery and source inspection.
- `onec-metadata`: bounded extraction of textual source from module containers.

## Impact

Gateway protocol dispatch, configuration resource acquisition and decoding,
and tests against real 1C configuration resources. The source inspection API
exposes configuration code to all authenticated users.
