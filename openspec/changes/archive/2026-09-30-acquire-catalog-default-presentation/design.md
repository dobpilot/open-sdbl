# Design

296 UNF owner Config descriptors are matched to native XML by UUID. Class 57 position 20 has value 0 for all 26 AsCode catalogs and 1 for all 270 AsDescription catalogs, without disagreements. The separately acquired VersionsOfExtensions resource is AsCode/0.

Add ConfigDescriptor.catalog_default_presentation: Option<CatalogDefaultPresentation> with AsCode and AsDescription variants. Preserve the scalar in the existing streaming parser; validate the same measured resource version, generated UUID prefix and owner header as catalog standard attributes. Unknown scalar or truncated layout gives None. Code settings and default presentation are separate acquired facts: do not derive one from the other and do not implement BSL presentation here.

Native XML from another configuration with the same UUID is not an oracle for this configuration. Existing fixtures receive only mechanical None initialization.
