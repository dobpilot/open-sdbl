# Preserve direct RLS session parameter identity

## Why
Native 1C 8.3.27.2342 initializes an unset session parameter read directly in an RLS predicate. Bound compilation currently loses that source context.

## What Changes
Preserve existing structured missing-session-parameter diagnostics for direct RLS parameter tokens. Keep ordinary query parameters unchanged.

## Impact
Internal query compilation; no dependencies or new public API. Version 0.9.30.
