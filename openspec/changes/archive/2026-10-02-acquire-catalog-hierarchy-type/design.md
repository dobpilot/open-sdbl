# Design

Reuse the existing runtime_metadata projection and brace Value parser. Native 1C 8.3.27.2342 Designer XML and matching fixture Config resources establish positions and codes; unknown layouts stay unknown.

For catalog classes56/57 with61 fields, native HierarchyType is position36 (folders/items=0, items=1), independently of Hierarchical position37. Existing catalog_owners already acquires position12 and needs no duplicate API; verify empty, single and multiple lists through snapshots. Other layouts are not generalized.

The demo export failed globally with Stream format error. Only independently parseable matching XML files provide evidence; no claim of a successful whole demo dump is made.

## Evidence

737 recognized native catalogs were checked by UUID: 296 UNF, 246 buh, 195 demo. Both hierarchy modes occur (659 folders/items, 78 items-only). Complete owner lists cover empty, single, two and three owners (674/55/5/3). Owner-header versions3/9,2/8 and2/10 are independently recognized for these fields; existing own-attribute projection remains strict. The existing public catalog_owners field is reused, with no second owner API.

## Validation

Workspace formatting, Clippy with denied warnings, tests (824 passed, 0 failed, 13 ignored), rustdoc with denied warnings and all strict OpenSpec validation passed. Existing assertion expectations were not changed; manual descriptor fixtures received only None initializers for the two new public fields. Version remains0.9.6.
