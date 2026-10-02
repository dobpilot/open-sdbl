# Fix acquired enumeration reference identities

## Why
Measured enumeration class 20 places EnumRef before EnumManager. The generic third-GUID projection incorrectly publishes the manager identity.

## What Changes
- Correct measured enum reference identities without altering document or catalog layouts.
- Preserve version 0.9.6 and add native XML corpus regressions.

## Impact
- Config metadata projection and regression fixtures; no new API or dependencies.
