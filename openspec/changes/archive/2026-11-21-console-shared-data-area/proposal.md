# Default console to the shared data area

## Why
The console cannot execute the same conditional-separator queries as the gateway and reports condition GUIDs without human-readable names.

## What Changes
- Bind conditional separator conditions to the shared area in console compilations and restriction derivation, including after refresh.
- Show known condition names alongside GUIDs in console diagnostics.
- Use the requested empty date for false conditional date separators, preserving explicit failures for other unsupported types.

## Capabilities

### Modified Capabilities
- `query-repl`: shared-area default and named condition diagnostics.
- `query-compilation`: empty date for a false conditional date separator.

## Impact
- Console session assembly, diagnostics, query compilation, tests and documentation. The empty date rule is an explicit requested behavior and is not independently measured against a live 1C platform.
