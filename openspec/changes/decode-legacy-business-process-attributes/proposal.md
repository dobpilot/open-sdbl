# Change: Decode measured legacy business-process attributes

## Why
Original BUH startup cannot resolve Автор because its BusinessProcess attribute collection uses wrapper 2/5 instead of the supported 3/7. A complete byte-preserving CF imported by native 1C and exported to XML now independently confirms all 27 members.

## What Changes
Accept the exact additional homogeneous 2/5 collection shape for BusinessProcess attributes, retaining complete-or-unknown validation and all existing member/type checks. No new public API.

## Impact
Business-process runtime metadata decoder and native paired fixture tests only. No version, dependencies or main checkout changes.
