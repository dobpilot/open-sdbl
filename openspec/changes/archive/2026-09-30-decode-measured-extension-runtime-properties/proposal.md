# Decode measured extension runtime properties

## Why
Configuration consumers cannot project native extension SafeMode, scope and HashSum from guessed values. Native 1C/database differential measurements now identify the safe-mode Boolean, profile string, InfoBase scope and the Base64 root generation hash.

## What Changes
- Decode known safe-mode/profile record shapes, preserving unknowns for other shapes.
- Expose a separately named platform hash sum, keeping the existing hex CAS content hash.
- Expose a typed measured InfoBase scope while retaining the raw scope code.

## Impact
Public metadata/database extension data, native fixtures and tests. No new dependencies.
