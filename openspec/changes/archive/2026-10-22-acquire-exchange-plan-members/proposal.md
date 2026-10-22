# Acquire exchange plan members

## Why
Native DEMO ExchangePlan resources contain typed own attributes, but the SDK does not expose their complete owner membership. File-source validation cannot distinguish known members from unknown composition.

## What Changes
- Add optional complete ordered own exchange plan attribute GUID membership to ConfigDescriptor.
- Decode only the paired native 37/51 owner and 4/7 attribute wrapper, using existing declared type projection and membership checks.
- Release version 0.9.43; no runtime exchange operations or dependencies.

## Impact
Additive metadata API; unknown and incomplete collections remain None.
