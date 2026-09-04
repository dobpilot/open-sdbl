## Why

The CLI security work intentionally left two test-infrastructure concerns out
of its runtime scope: many legacy tests still live in `main.rs`, and the query
fuzz fixture cannot reach dereference or tabular-section compilation paths.
Keeping these concerns in a focused follow-up lets the completed security
contract be archived without losing the remaining maintainability work.

## What Changes

- Move root CLI tests beside the modules that own the behavior and remove
  duplicate coverage.
- Extend the query compiler fuzz snapshot with references and tabular sections.
- Compile the fuzz workspace in CI without running an unbounded fuzz campaign.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `crate-architecture`: CLI tests follow the production module boundaries.
- `query-compilation`: fuzz coverage reaches metadata-dependent query paths.

## Impact

No public API or runtime behavior changes. The change affects test placement,
fuzz fixtures, and CI verification only.
