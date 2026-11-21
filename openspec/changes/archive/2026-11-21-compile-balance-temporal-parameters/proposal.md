# Compile temporal balance parameters

## Why
The configured host cannot express a document recorder boundary without losing its reference. Native 8.3.27.2342 controls compare 60 manager/query balance observations for three recorders of two document kinds.

## What Changes
Add typed point-in-time and date/point boundary parameter values to the existing compiler. Support the measured accumulation Balance period argument, preserving date, reference type, physical RRRef and inclusion. Explicitly reject these values in unsupported expression positions. Add a point-in-time variant to the existing logical result recipe without changing physical projection.

## Impact
Public Rust API and query compilation. No new dependencies, connections or SQL pipeline. Existing scalar output stays unchanged.
