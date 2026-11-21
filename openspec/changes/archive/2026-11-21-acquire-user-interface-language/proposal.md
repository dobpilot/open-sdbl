# Acquire selected user interface language

## Why
Hosts cannot freeze the authenticated user language because the existing user decoder drops its measured identity.

## What Changes
Expose an optional selected-language GUID in UserData, preserving known nil and unknown layouts separately. Release SDK packages as 0.9.24.

## Impact
User decoding only; both database paths reuse the decoded result without new SQL or dependencies.
