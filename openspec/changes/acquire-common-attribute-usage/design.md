# Design

Reuse the existing runtime_metadata projection and brace Value parser. Native 1C 8.3.27.2342 Designer XML and matching fixture Config resources establish positions and codes; unknown layouts stay unknown.

For common attributes expose raw typed AutoUse plus full Content, avoiding an inferred applicability algorithm. Preserve common versus own membership. AutoUse Use=0, DontUse=1; measured Content Use=1, DontUse=2. Unmeasured Auto content entries remain unknown. Class5 has15 fields; Pattern belongs to its nested field header.

The demo export failed globally with Stream format error. Only independently parseable matching XML files provide evidence; no claim of a successful whole demo dump is made.

## Evidence

The compact oracle in tests/fixtures/session-shape-096 matches 16 complete native XML files and 1584 Content entries. Seven files each belong to buh and UNF, two to demo. The separate demo НаименованиеЯзык1 fixture retains all 51 Config entries and independently verifies the named catalog link and native string qualifiers. Other demo Content UUID links whose XML is absent are not called independently verified. Common attribute values, BSL setters and applicability outside the acquired explicit facts are not implemented by this SDK API.

## Validation

Workspace formatting, Clippy with denied warnings, tests (824 passed, 0 failed, 13 ignored), rustdoc with denied warnings and all strict OpenSpec validation passed. Existing assertion expectations were not changed; manual descriptor fixtures received only None initializers for the two new public fields. Version remains0.9.6.
