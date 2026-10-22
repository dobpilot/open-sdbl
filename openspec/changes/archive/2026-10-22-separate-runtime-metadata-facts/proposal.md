# Change: Separate runtime metadata facts from query snapshot finalization

## Why
ORM materialization repeatedly builds complete query snapshots although it consumes only runtime facts. Measured session startup repeatedly pays for physical field/query index/report/fingerprint finalization.

## What Changes
- Add a runtime-only metadata facts view and cached resolution API.
- Reuse the same descriptor normalization, extension merging, object/value construction and strict separator validation.
- Keep full query snapshot fingerprints, reports and prepared-query ownership unchanged.

## Impact
Public additive API in open-sdbl and open-sdbl-db; version 0.9.39. No new dependencies or I/O.
