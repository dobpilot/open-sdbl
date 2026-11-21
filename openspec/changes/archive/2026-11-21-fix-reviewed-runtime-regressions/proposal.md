# Fix reviewed runtime regressions

## Why
Review found four regressions: the default extension decoder lost its output
ceiling, cancelling MSSQL acquisition can leave a reusable connection with an
unfinished transaction, PostgreSQL equality rules depend on expression shape,
and source-free projections can advertise a recipe after losing Undefined.

## What Changes
- Restore the default extension decoded-byte ceiling while retaining explicit limits.
- Disconnect cancelled MSSQL metadata acquisitions throughout their transaction lifecycle.
- Apply the existing measured PostgreSQL scalar equality rule to direct operands
  and source-free expressions, preserving contextual CASE and provider boundaries.
- Withhold source-free logical recipes when SQL loses a distinguishable Undefined value.
- Add regression coverage; preserve public signatures and crate dependency boundaries.

## Impact
Affected capabilities: onec-metadata, crate-architecture, query-compilation.
Implementation is a bounded repair; broader internal restructuring is proposed
separately in `refactor-compiler-and-metadata-boundaries`.
