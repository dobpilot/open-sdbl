# Acquire catalog default presentation

## Why
A host needs the configured catalog DefaultPresentation to choose Code or Description. CodeType and lengths do not establish this choice.

## What Changes
Expose an optional measured catalog default presentation on ConfigDescriptor, independent of code type and lengths. Preserve unknown settings. Version remains 0.9.5.

## Impact
Public metadata API; no BSL implementation, SQL inference or new dependencies.
