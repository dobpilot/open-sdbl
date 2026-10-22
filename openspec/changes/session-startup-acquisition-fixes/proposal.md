## Why
Original session startup requires connection-bound transactions and independently valid functional-option fields with measured populated Content.

## What Changes
- Add explicit read-only session transaction lifecycle to async and blocking providers.
- Accept measured populated functional-option Content while rejecting malformed forms.

## Impact
Database SDK session API and metadata acquisition; no dependencies or BSL evaluation.
