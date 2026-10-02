# Change: Acquire tabular-section own attributes

## Why
Consumers need complete ordered row ownership from Config without guessing from flattened descriptors.

## What Changes
- Add `ConfigDescriptor::tabular_section_attributes: Option<Vec<Guid>>` for measured native tabular-section collections.
- Preserve unknown declarations and incomplete membership as unknown.

## Impact
- Affected spec: onec-metadata.
- Public additive metadata API, version remains 0.9.6.
