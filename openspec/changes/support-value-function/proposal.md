## Why

Real 1C queries use `ЗНАЧЕНИЕ` to address enumeration values and predefined
catalog items by stable metadata names. The compiler currently parses the
function name as a field, while metadata acquisition ignores the catalog
`.1c` Config resources that contain the authoritative predefined names and
GUIDs.

## What Changes

- Recognize bilingual `ЗНАЧЕНИЕ`/`VALUE` expressions with the strict
  `<kind>.<object>.<value>` path shape.
- Decode catalog predefined values from part-zero `<object-guid>.1c` Config
  resources and index them together with enumeration values already present
  in bare-GUID Config resources.
- Generate physical 1C GUID bytes using the platform UUID field order.
- Compile enumeration values as binary constants and catalog predefined values
  as indexed `_PredefinedID` lookups returning the catalog `_IDRRef`.
- Support PostgreSQL and MSSQL without adding I/O or dependencies to the root
  crate.
- Diagnose unsupported kinds, missing objects or values, ambiguity, and absent
  physical catalog columns before execution.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: acquire and resolve authoritative predefined-value metadata.
- `query-repl`: compile bounded `ЗНАЧЕНИЕ`/`VALUE` expressions.

## Impact

- Extends Config acquisition by the small set of `.1c` resources in addition
  to existing bare-GUID resources.
- Extends public metadata structures with predefined values and lookup APIs.
- Generated catalog lookups remain read-only and use the platform-maintained
  `_PredefinedID` index.
