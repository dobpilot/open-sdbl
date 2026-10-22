# Change: Resolve explicit configuration resources

## Why
Indexed resource reads alone cannot build query-ready metadata without the existing full Config acquisition. Discovery also needs resource existence and own names without decoding every object's attributes.

## What Changes
- Add reusable physical discovery inputs without Config or ConfigCAS scans.
- Resolve only caller-provided Config resources and explicit extension structures using existing decoders and resolver.
- Add bounded filename-only discovery and own-header-only decoding.
- Preserve separator identity, GUID identities and existing acquisition behavior.
