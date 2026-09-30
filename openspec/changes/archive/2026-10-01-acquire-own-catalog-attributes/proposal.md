# Acquire own catalog attribute identities

## Why
Hosts compiling an original catalog object module need the catalog's own attribute names. Flat nested Config descriptors do not distinguish the owner attributes from tabular-section attributes, and SQL is not an ownership oracle.

## What Changes
Expose ConfigDescriptor.catalog_attributes as an optional complete ordered list of GUIDs on the catalog owner. Preserve unknown and incomplete lists instead of returning partial membership. Reuse existing nested descriptors for names and type references. Version remains 0.9.5.

## Impact
Public metadata API and measured Config projection; no BSL type semantics, SQL fallback or new dependencies.
