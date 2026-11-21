# Decode the measured legacy Boolean constant declaration

## Why
BUH conditional separation references a constant whose native XML and CF declare Boolean, but class14/12 has no projected declaration.

## What Changes
Recognize only the measured class14/12 Boolean constant profile using strict local validation and existing identity and Pattern parsing.

## Impact
Only metadata projection changes; no API, dependencies, SQL, rights or separation policy changes.
