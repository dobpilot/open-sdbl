# Design

Use the existing runtime_metadata projection and Value parser. Native XML/Config confirms class27/23 attribute position13 for own, section and common attributes. Catalog classes56/57 with61 fields store the standard property list in field45. A complete measured list contains all nine standard names in native order; sparse or unsupported tables stay unknown. Property2723eb98-b4c1-498a-a6f3-70444757902f carries enum98ea8e5a-b586-442b-b944-6e3447734aa7 with0 DontCheck and1 ShowError. Other codes stay unknown. Recognize only measured versions/counts and preserve owner membership; never substitute inferred Code or Description defaults.

The native corpus originates in 1C8.3.27.2342 Designer XML and existing Config fixture packs. The partial demo export contributes only whole parseable XML files; no claim of a successful full demo export is made. Behavioral checking and handlers remain in open-bsl.

## Evidence and limits

The corpus matches 728 complete ordered standard tables and 31334 named field settings (including 27 common attributes) to native XML by UUID. Both DontCheck and ShowError occur. Original buh СтраныМира declares ShowError for both Code and Description; a newly authored targeted assertion initially guessed DontCheck for Code from an unrelated runtime probe, failed, and was corrected to the saved XML. The parser did not change to satisfy it. Existing assertion expectations remain unchanged. Sparse table semantics are not inferred: the API conservatively returns unknown for unverified completeness. Only recognized root classes5/40/56/57 are projected; other formats and modes remain unknown. No checking method, handler, numbering or message behavior is implemented.

## Validation

Formatting, Clippy with warnings denied, workspace tests (828 passed, 0 failed, 13 ignored), rustdoc with warnings denied and all strict OpenSpec validation passed. Existing manual descriptor fixtures only received None initializers for new fields; their assertion expectations were not changed. Version remains0.9.6.
