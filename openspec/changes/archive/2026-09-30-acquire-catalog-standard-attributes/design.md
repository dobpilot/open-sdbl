# Design

Native UNF XML and UUID-matched Config descriptors establish catalog class 57 positions 17 CodeLength, 18 CodeType (0 Number, 1 String), 19 DescriptionLength. 296 matching acquired corpus descriptors agree; the separately read VersionsOfExtensions descriptor has Number, 9, 0.

Expose one optional CatalogStandardAttributes record on ConfigDescriptor, with CatalogCodeType and unsigned lengths. Zero length is retained as configuration metadata and does not determine BSL property availability; unknown class/form/type does not become an absent attribute. Parse only the owner resource and the measured class layout with validated generated UUID prefix and owner header. Do not inspect SQL or infer hierarchy, owners or defaults. The streaming parser retains three scalar settings instead of introducing a second full-tree parse.

Existing SchemaStorage exposes Code/Description tags and Parent/Owner column presence but not configured hierarchy/owners or lengths. MetadataObject currently retains only live-SQL allowed length. AcquiredConfiguration retains Config resources. Consumers decode their owner descriptors through the existing parse_config_descriptor API, so no duplicate snapshot fields are needed.
