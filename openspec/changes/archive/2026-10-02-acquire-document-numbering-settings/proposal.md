# Change: Acquire document numbering settings

## Why
Runtime consumers need verified NumberPeriodicity and the configured document numerator identity without inferring either from SQL or unrelated numeric fields.

## What Changes
- Add an optional complete document numbering-settings value with periodicity and an optional numerator object identity.
- Keep existing number type/length settings unchanged.
- Distinguish unknown acquisition from verified absence of a numerator.

## Impact
- Affected specification: onec-metadata.
- Additive public metadata API; version remains 0.9.6 by explicit instruction.
- No number reservation, SQL writes, authorization or runtime semantics.
