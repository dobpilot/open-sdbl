# BUH controlled-transactions document reconnaissance

Read-only native CF/XML evidence for `ПрочиеКонтролируемыеСделки`, owner `c63f584a-d27d-42f2-b656-79f7a64d9e93`. No database connection, SDK/runtime edit, version change or build was performed; only small standalone consumers were linked to the frozen SDK rlib.

The owner is measured class38/51 with identity1/7, empty String comment and flags0/0. Nine own attributes use wrapper4/5; one section `Сделки` contains18 fields in wrapper7/5. All29 identities use the already recognized1/7 shape. Two own comments are nonempty Strings, exactly matching native XML: СуммаДокумента and Комментарий. All other comments are empty.

The current staged parser rejects these two headers because legacy_data_processor::identity requires an empty comment. Therefore complete own membership is unknown and the dependent whole section projection is also unknown. Public native parsing returns29 descriptors but no declared types/memberships.

Causal control empties only the two comment Strings; exact AST diff paths are in causal-controls.json. The same frozen public SDK then returns all nine owns, one section and18 fields, with32 independently XML-derived qualified type rows/references and complete ordered UUID/name membership. This synthetic mutation is not a claim of native data. Native artifacts remain unchanged.

Minimal proposed fix: allow a String comment locally in the measured legacy document38 identity admission while preserving all existing reference/name/synonym/flag checks and atomic publication. Do not globally relax shared legacy identity, alter offsets, or add new type decoding. Existing declaration/section helpers already handle the whole native profile once own admission succeeds.
