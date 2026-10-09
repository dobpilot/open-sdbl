# Respect data-separator usage in 0.9.13

## Why
Native 1C 8.3.27.2342 rejects a table with an Independent separator whose use flag is false. The SDK currently removes its predicate and permits an unrestricted read. Lazy Session hosts also need a structural request for the use flag before a required value.

## What Changes
- Reject disabled Independent separators before SQL execution in the shared compiler.
- Measure IndependentAndShared independently and preserve its verified behavior.
- Resolve missing bound use flags before value parameters, only for tables actually read; keep preparation value-free.
- Publish workspace version 0.9.13.

## Impact
Query-compilation behavior and missing-session-parameter diagnostics; no new dependencies or provider-specific SQL path.
