# Acquire measured filling-check settings

## Why
Original object module initialization requires native FillChecking metadata; object values or BSL text cannot determine those settings.

## What Changes
- Expose typed optional DontCheck and ShowError settings on acquired attribute descriptors.
- Expose a complete ordered optional catalog standard-attribute settings list.
- Add independent native Config/XML and corrupted-form regressions.

## Impact
- Public metadata Rust API, version remains0.9.6.
- No implementation of BSL checking, handlers, messages, database writes or new dependencies.
