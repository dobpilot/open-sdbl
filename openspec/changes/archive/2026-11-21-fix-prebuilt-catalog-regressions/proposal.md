# Fix prebuilt catalog and metadata resolution regressions

## Why
Catalog prebuilding loses columns when separators require session values, can
block gateway refresh, and may report the wrong database for embedded gateways.
Metadata resolution also assumes threads exist on every target.

## What Changes
- Build catalog descriptions with synthetic separator values without changing query authorization.
- Move asynchronous catalog construction to a blocking worker and use the configured database name.
- Bound metadata finalization workers and fall back to sequential execution without thread support.
- Cover the regressions and avoid repeated sleeps in the ordering test.
