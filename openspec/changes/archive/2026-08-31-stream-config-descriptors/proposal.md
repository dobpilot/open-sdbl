## Why

The reported information base contains enough bare-GUID Config resources that
building and dropping a complete generic `Value` tree for every blob still
takes more than two minutes after lower-level decoder optimizations. The CLI
needs only descriptor windows and inherited collection purpose from those
trees.

## What Changes

- Project Config descriptors while validating brace serialization.
- Retain only a four-value sliding window per list plus simple scalar lists
  needed for self-references and synonyms.
- Preserve owner/nested descriptor order, inherited field purpose, synonyms,
  comments, malformed-input rejection, and public generic parsing APIs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: avoid complete intermediate value trees for Config
  descriptor acquisition.

## Impact

- Internal Config parsing performance and peak memory.
- No accepted-resource, public API, dependency, or I/O change.
