# Change: Clarify configuration fingerprint scope

## Why
Whole-acquisition documentation still recommends CONFIG_FINGERPRINT as a detector for every configuration change, while both providers hash only the metadata filter and no extension tables.

## What Changes
- State the exact bare-GUID and .1c/.9/.7 Config boundary.
- Remove whole-configuration and extension freshness promises from documentation.
- Preserve existing SQL and API; no full fingerprint or allocation feature.

## Impact
- Affected spec: crate-architecture.
- Corrected compatibility promise; version remains0.9.6.
