# Evaluate metadata rights before privileged bypass

## Why
Hosts need to distinguish invalid targets and inapplicable rights from a valid refusal before applying privileged mode. Native catalog standard-right observations prove role-local parent gating and default behavior.

## What Changes
- Add pure structural resolution and applicability of supported catalog metadata Read/View rights.
- Add complete cached-role evaluation without SQL, RLS compilation, journal delivery or privileged-mode state.
- Publish version 0.9.22; retain existing SQL authorization unchanged.

## Impact
- Affects deterministic metadata API and database AccessStore.
- Adds no external dependencies, connection or mutable global cache.
