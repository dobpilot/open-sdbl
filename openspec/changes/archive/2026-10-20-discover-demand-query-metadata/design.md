# Design
The query API returns owned kind/name pairs and constants None/Named/All through an exhaustive private AST visitor. It uses the existing tokenizer/parser, limits, positional diagnostics, kind aliases and folding. This is direct-reference discovery, not a complete transitive or RLS dependency closure.

ConfigHeader exposes ConfigTypeIdentity Unknown/Absent/Known(Guid) and optional section identities. Only existing measured cases are used; the implementation reuses shared identity validators with the detailed decoder. Raw serialization may be traversed but no attribute ConfigDescriptor or runtime ORM object is constructed. Unneeded attribute collections are discarded by identity-only serialization projection. Unknown layouts do not become absent identities.

Extension discovery reuses the existing _ExtensionsRestruct reader and decoder in a read-only acquisition with cached layout. It does not read Config/ConfigCAS and does not alter ConfigurationMetadataInputs construction. Existing bounds, session closed/context checks and cleanup apply. No live DB, new dependencies, or old expected-result changes.

All-pack parity exposed an existing ConfigParser fallthrough for unsupported primary prefixes: demo DocumentJournal 49880f4a-1155-4129-9d59-bf95b7e8020e receives ffcb446e-6814-4f62-9e20-217599155b0f from an unrelated root collection. The new projection leaves unsupported primary forms Unknown instead of copying that GUID. The old detailed decoder is deliberately not repaired here. Parity checks compare supported Known identities and all available section membership, not an asserted identity for unsupported classes.

The declaration projection also preserves own localized synonyms, enum member GUID/name/synonyms, and the existing measured business-process route-point reference GUID. These are immutable declaration/presentation facts needed before detailed hydration, not new attribute metadata or inferred type layouts. Existing Synonym and header validators remain authoritative.

The rolling identity scanner also retains deduplicated recognized member GUIDs including the owner's GUID, giving a member-to-resource ownership index without attribute/type decoding. Native parity compares these GUID sets with detailed decoder descriptor identities.
