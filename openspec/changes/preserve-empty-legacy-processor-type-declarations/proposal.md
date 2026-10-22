# Preserve measured empty legacy processor declarations

## Why
BUH conversion processor metadata contains four valid empty Pattern declarations. Rejecting them discards all 43 own attributes, including three independently typed ValueTable attributes.

## What Changes
Preserve native textual comments and explicit empty declarations as Some(empty) in the measured legacy processor profile. Keep unsupported or malformed declarations unknown and retain complete-or-unknown membership. No runtime interpretation or public API changes.

## Impact
Only metadata decoding for the measured legacy processor attribute profile and native regression fixtures.
