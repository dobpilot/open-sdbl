# Design

The private SQL ReferenceCategory table already holds ten native IDs measured on 1C 8.3.27. Rename its semantic enum to the public ReferenceTypeSet and reuse the same lookup and membership rules. ConfigDeclaredType::reference_type_set recognizes only # declarations with a known family reference. None means no recognized family, not a complete or empty concrete declaration. Raw GUIDs and primitive qualifiers are preserved.

Named aliases already expand to cloned original declarations. A generic member survives that expansion and remains recognizable by the new method. Unknown GUIDs, unavailable aliases, cycles and duplicate definitions retain their existing semantics; no partial declaration or empty successful type list is invented.

Automatic expansion in the SDK is deliberately rejected: selected runtime facts contain only reached objects. Consumers with a complete configuration reference catalog can union concrete members and known families, deduplicate overlapping families/aliases, and reject a list containing an unresolved member. They do not decode native GUID constants. This change does not acquire additional resources or infer family completeness.

Paired native Config/XML/EDT evidence for DEMO Catalog.ИдентификаторыОбъектовМетаданных.ЗначениеПустойСсылки lists eight families in the same source order. The existing Any mapping covers the nine reference-bearing metadata kinds, including Enumeration, exactly as before in SQL.

The public Kind value can be constructed with a nonreference MetadataKind. Its membership is intersected with the single Any universe, so Kind(Constant).contains_kind(Constant) is false and no synthetic reference family is invented.
