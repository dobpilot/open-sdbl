# Change: Acquire business-process route points

## Why
Configured BSL hosts require complete route-point identities and names. Unknown membership must not appear empty.

## What Changes
- Acquire complete native business-process route-point membership and its separate generated reference type.
- Preserve existing descriptor identities and owner relationships.
- Publish version 0.9.8 with native compiler/XML fixtures.

## Impact
Public metadata API is additive; descriptor struct initializers require the new fields. No dependencies or BSL implementation are added.
