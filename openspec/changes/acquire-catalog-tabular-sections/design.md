# Design

Add `ConfigDescriptor.catalog_tabular_sections: Option<Vec<Guid>>` only on the resource owner. The measured root collection is `932159f9-95b2-4e76-a8dd-8849fe5c5ded`; the section header has relative path `[0,1,5,1]` inside each entry. Use the existing streaming decoder with a private path marker, excluding nested fields. Validate declared count, unique non-nil identities and every header. Unknown or incomplete collections yield None; known empty collections yield Some([]).

296 native UNF XML catalogs match Config: 313 sections, 151 empty collections. No database writes or BSL value semantics are involved. Version stays 0.9.5.
