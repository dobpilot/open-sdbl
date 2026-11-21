# Keep DBeaver column introspection within catalog budgets

## Why
DBeaver's query for one relation's columns exhausts the allocation budget on a populated catalog before the relation filter can reduce the intermediate rows.

## What Changes
- Evaluate the recorded attribute introspection query efficiently with its relation restriction while retaining existing catalog budgets and SQL semantics.

## Capabilities

### Modified Capabilities
- `pgwire-catalog`: DBeaver column introspection on populated catalogs.

## Impact
- Catalog evaluation and regression tests; no limit increase.
