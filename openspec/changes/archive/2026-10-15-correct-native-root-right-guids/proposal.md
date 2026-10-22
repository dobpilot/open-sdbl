# Change: Correct measured configuration right identifiers

## Why
Native single-right roles show four wrong GUID-to-name associations in RIGHTS. SaveUserData is decoded as ConfigurationExtensionsAdministration, causing a false session-startup refusal.

## What Changes
Correct only the four independently measured identifiers using native role resources. Preserve all names and public signatures and retain unassigned identifiers as Other.

## Impact
The existing RIGHTS table, native fixtures and focused tests. No dependencies, I/O or new parsing mechanism.
