# Decode the measured legacy functional-option fields

## Why
Original BUH initialization reaches a functional option whose mode and location are unknown despite an unchanged native CF/XML declaration. Existing supported modern options already preserve those fields.

## What Changes
Recognize the measured old tag1/seven-field functional-option identity through a local branch. Reuse strict identity, counted Content and Boolean flag helpers; publish location and mode together only after complete validation.

## Impact
No public API, dependencies, rights, SQL or host fallback changes. Main and frozen parent remain unchanged at version 0.9.20; this is an isolated review, not a release.
