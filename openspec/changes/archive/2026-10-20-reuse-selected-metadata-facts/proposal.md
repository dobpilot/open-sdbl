## Why
Demand-loaded consumers repeatedly parse the same immutable selected resources after every newly reached object. Pure resolution also repeatedly scans schema/object names even though the equality rule already has a matching hash key.

## What Changes
- Add a small ConfigurationMetadataCache admitting only new immutable resources under ordinary limits and resolving accumulated parsed facts through the existing strict path.
- Preserve the existing stateless function, outputs, fingerprints and prepared-query validation.
- Replace service-table and dependency-owner repeated scans with equivalent per-resolution indexes.
- Bump packages to 0.9.34 without new dependencies.
