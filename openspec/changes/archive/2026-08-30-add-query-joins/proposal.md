## Why

The bounded compiler rejects every explicit join. Users need bilingual INNER,
LEFT, RIGHT, and FULL joins between resolved 1C metadata sources. FULL JOIN
must be expressed as a union of directional joins rather than emitted as a
native PostgreSQL FULL JOIN.

## What Changes

- Parse one two-source bilingual INNER, LEFT, RIGHT, or FULL JOIN in a SELECT
  branch.
- Resolve qualified fields from both authoritative metadata sources.
- Generate native INNER/LEFT/RIGHT joins; transpose FULL into two LEFT JOIN
  branches combined by UNION ALL with an anti-match predicate.
- Apply WHERE, DISTINCT, TOP, and final ordering to the logical full-join
  result, accept repeated trailing semicolons, and reject unsafe join shapes
  before execution.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: add bounded two-source JOIN compilation, including FULL JOIN
  via UNION ALL.

## Impact

- Dependency-free SDBL AST, parser, metadata resolution, and PostgreSQL
  generation.
- The CLI still executes one generated read-only statement.
