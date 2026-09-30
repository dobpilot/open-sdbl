# Acquire catalog standard attributes

## Why
Hosts creating catalog objects need the configured type and presence of Code and Description. Current Config descriptors discard these settings and SQL declarations are not a substitute for configuration metadata.

## What Changes
Expose measured catalog CodeType, CodeLength and DescriptionLength on the owner Config descriptor. Unknown layouts remain unknown. Version remains 0.9.5.

## Impact
Public metadata descriptor API; no SQL, dependencies or BSL execution.
