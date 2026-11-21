# Change: Decode measured legacy BUH data processor metadata

## Why
Original BUH startup loses its own Организация attribute because the native data processor uses class 16/12 and identity 1/7. A retained native CF/XML pair establishes complete membership.

## What Changes
Recognize the measured legacy data processor shape locally, retaining complete-or-unknown validation for own fields and sections. No public API or dependencies change.

## Impact
Data processor metadata projection and native fixture tests. Release 0.9.18.
