# Expose defined-type identity in bounded headers

## Why
Selective consumers cannot discover the native UNF defined type behind a declaration because bounded Config headers omit its generated type GUID.

## What Changes
- Add a measured defined-type identity to ConfigHeader with the existing unknown/absent/known distinction.
- Share recognition with full decoding and verify native parity without changing resolution semantics.

## Impact
Public metadata API; SDK packages version 0.9.37. No new dependencies, SQL or acquisition operations.
