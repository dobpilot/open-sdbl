# Design

Native XML and acquired Config are matched by UUID for 296 UNF catalogs. Root collection cf4abea7-37b2-11d4-940f-008048da11f9 contains exactly 3540 own attributes in native order. The 1453 tabular-section attributes are separate; 145 catalogs have tabular-section attributes and 19 have a known empty own-attribute collection. ExternalUsers has eight own attributes including d65c4485-7bac-4206-9a6c-7156f4bb876d; its AdditionalAttributes tabular section has three other attributes under collection 888744e1-b616-11d4-9436-004095e12fc7.

Add only ConfigDescriptor.catalog_attributes: Option<Vec<Guid>> on the owner. Some requires exactly one measured root collection, a recognized catalog owner prefix, an unsigned declared count matching actual entries and identities, and exactly one recognized nonempty descriptor with non-nil distinct GUID per entry. Unknown, duplicate or incomplete membership produces None. Some(empty) is a known empty collection. Nested descriptors retain None. Consumers resolve the GUIDs in existing nested descriptors.

Use the current streaming decoder and its existing descriptor projection. A collection tracker records the per-entry descriptor delta; it does not parse a second tree or scan SQL/source. No new public field-purpose variant or general owner/path model is required. Attribute value defaults and type semantics remain outside this change.
