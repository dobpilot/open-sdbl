# Reuse resolved runtime metadata during query finalization

## Why
Selected callers currently normalize accumulated metadata twice when a query follows runtime hydration.

## What Changes
Expose query finalization from immutable resolved runtime facts. Preserve full snapshot fields, indices, reports, fingerprints and strict descriptor diagnostics. Bump SDK to 0.9.40.

## Impact
Core metadata and database parsed-resource cache; no new dependencies, SQL, platform semantics or byte format.
