# Change: Authorize measured accumulation register field reads

## Why
Native 1C 8.3.27.2342 measurements prove dimension/resource programmatic reads follow owner Read, independently of UI View and attribute defaults. The current SDK rejects these verified fields.

## What Changes
- Authorize verified own dimensions and resources through existing field checks.
- Preserve existing object Read, selected-user, role completeness and RLS enforcement.
- Release workspace version 0.9.10 without new dependencies.

## Impact
Affected specification: access-rights. Affected code: database field access and tests. No additional permission system or public signature is introduced.
