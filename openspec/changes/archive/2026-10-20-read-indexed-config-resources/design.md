# Design

`ConfigurationBootstrap` carries layout, compressed root, compressed descriptor and raw `ExtensionCatalogRow` values. `configuration_bootstrap(selector)` reuses `ConfigSelector::root_resource_name` and `configure`; bootstrap reads only those two Config resources and the extension catalog. It does not read DBNames, SchemaStorage, live tables, extension resource bodies or resolve MetadataSnapshot. The caller owns dependency discovery and decoding, including resource decompression limits.

`configuration_resources(&[&str])` selects names with an escaped literal SQL IN predicate against indexed physical filename, not a trimmed or cast column. `extension_resources(&[ContentKey])` selects measured physical CAS keys. Both return ConfigResource, preserve first-request order, deduplicate repeated names and reject missing names or broken parts. Empty requests return no resources without reading tables, while session state and blocking async-context checks still apply.

Layout is cached per provider session after successful acquisition. Every nonempty operation uses an independent read-only acquisition transaction with existing rollback/SQL Server poisoning. READ COMMITTED is not an atomic snapshot across operations; no fingerprint is recomputed. Each call independently enforces existing resource and retained-compressed-byte limits, including before extending an assembled resource. Bootstrap counts its two retained resources together. The extension catalog remains eager as before; ConfigCAS resources are never eager. Raw-resource reads do not decompress bytes; consumers reuse existing bounded decoders.

No external dependencies, language semantics, existing acquisition behavior or live database writes change. New comments and public rustdoc are Russian.
