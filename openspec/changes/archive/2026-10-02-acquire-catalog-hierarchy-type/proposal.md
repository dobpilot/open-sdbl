# Acquire catalog hierarchy type

## Why
Original configuration module initialization requires native acquired object context rather than guessed global names.

## What Changes
- Expose measured catalog hierarchy type through the existing metadata pipeline.
- Add native XML/Config corpus and malformed-form regressions.

## Impact
- Metadata descriptors and their public reexports; version remains 0.9.6.
- No new SQL path, database writes or external dependencies.
