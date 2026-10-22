# Design

`configuration_metadata_inputs()` on async and blocking sessions returns `ConfigurationMetadataInputs { layout, db_names, schema, live_tables }`. These are physical discovery indexes, read once by the caller and reused across hydration; Config, ConfigCAS and extension restructure bodies are not read. Existing provider MetadataSource readers, transaction cleanup, timeout and known-layout reuse remain authoritative.

`resolve_configuration_resources(&inputs, resources, extensions)` is synchronous and performs no I/O. It uses existing bounded resource decoders and the existing metadata resolver. Only explicitly passed resources are decoded; related descriptors including common attributes, separators and named types must be supplied by the caller. Physical inputs are projected before resolution to selected descriptor GUIDs, owning tables, service tables and used fields. Separator flags survive projection even when a setting descriptor is missing; unknown settings never become no separation. Supplied extension structures are explicit and never trigger a CAS fallback.

`configuration_resource_names()` reads only DISTINCT physical Config names, bounded by existing resource-count and retained-byte limits, never BinaryData. Empty stores return an empty list. The same session caches layout.

`parse_config_header_bounded` uses the existing measured descriptor self-reference/name identity rule and existing lexical routines. Its lightweight traversal never materializes ConfigDescriptor, attributes or runtime metadata. Kind remains derived from the existing configuration class index. Unknown headers return None; malformed serialization and decompression limits fail.

ObjectId and AttributeId already encode GUID bytes, and standard FieldId values are fixed. Hydration cannot renumber them. A regression test exercises one role-only AccessStore against two growing/reordered query snapshots without reset. Prepared query fingerprints still bind to the immutable snapshot used during preparation; callers reprepare after changing the snapshot.

Physical DBNames/SchemaStorage/live discovery may read the full indexes; it is not a full logical Config decode. READ COMMITTED provides no atomic snapshot across calls. Existing whole acquisition and tests remain unchanged. No external dependencies or live database writes.

A focused regression demonstrated that the existing whole resolver can report SeparatorSettingsMissing while a caller ignoring the report can compile an unfiltered query. The new explicit-resource helper therefore rejects missing separator and bound parameter descriptors with machine-readable ConfigurationDescriptorsMissing GUIDs before exposing a snapshot. Already supplied unsupported settings are a data error, avoiding an endless retry. The old resolver and query compiler remain unchanged.
