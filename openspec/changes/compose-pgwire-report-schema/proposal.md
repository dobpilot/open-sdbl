# Compose report schema lookups through pgwire

## Why

A report variant stores a reference to a metadata identifier. Clients need
to follow that reference and read the report's configured main data
composition schema without reconstructing its path from a template name.

## What Changes

- Add `onec_get_attribute_text(link, path)` for reading one attribute or
  following a dotted path of references.
- Add `onec_get_metadata_ref(name, property)` to resolve metadata
  references, starting with a report's main data composition schema.
- Allow scalar text service calls to be nested in arguments to pgwire
  service functions.

## Capabilities

### Modified Capabilities

- `pgwire-gateway`: compose attribute reads and metadata references.

## Impact

Service call parsing, authorized object lookups, and the base configuration
metadata index. Uses the existing data providers and XML resource reader.
