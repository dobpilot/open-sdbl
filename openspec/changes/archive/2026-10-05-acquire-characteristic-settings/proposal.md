# Acquire characteristic settings

## Why
The consumer cannot create characteristic objects from acquired configuration: value types, lengths and complete own attributes are unavailable.

## What Changes
Expose measured characteristic settings and complete ordered own attribute identities, preserving unknown versus known empty values. Preserve qualified types and expand acquired named types through the existing resolver. Release SDK 0.9.7.

## Impact
Public metadata descriptor API; configuration projection only. No new dependencies or database writes.
