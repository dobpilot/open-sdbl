# Design

RuntimeMetadataFacts is a distinct owned immutable type, not a partial MetadataSnapshot. It exposes normalized descriptors, objects, object identity lookup, schema, values, separators, criteria and roles. It has no fingerprint, query index or ordinary field accessor. MetadataSnapshot owns these common facts and exposes runtime_facts() by reference. Existing snapshot accessors and query preparation remain unchanged.

Selected physical projection and common normalization/object/value construction are shared private functions. Runtime resolution skips ordinary field construction, query indexes, comparison reports and fingerprint finalization. Separator fields are still built by the same field constructor because they carry strict settings and bound parameter names. Both database cache paths perform the same missing/unsupported separator validation before publishing facts.

This removes unnecessary query finalization during ORM growth; it does not promise delta-only descriptor resolution. Previously returned snapshots remain immutable. Full query resolution always computes the existing complete fingerprint over the exact ordered facts and retains Prepared ownership semantics.

Tests compare every runtime-consumed fact against complete snapshots over growing demo/buh/unf fixture sets, plus extension/named-type/separator/error controls. Alternating bounded fixture benchmarks compare the existing complete path against runtime-only resolution. Existing tests are not changed.
