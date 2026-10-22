# Design

Runtime facts retain private extension context necessary for deferred ordinary fields and diagnostic reports. Borrowed finalize_query clones the owning value once while sharing immutable descriptor, object, value and live-table vectors and their lookup/context maps through Arc; into_query_snapshot consumes already owned facts without that copy. Finalization constructs ordinary fields and query indices, and applies the existing fingerprint algorithm. It never selects resources, merges extensions, resolves named types or normalizes descriptors again.

The cache finalization method validates separator dependencies against admitted resource GUIDs before invoking the core finalizer. The facts are a complete immutable input: subsequent cache growth does not mutate them or silently add new resources. The caller chooses the matching current facts.

Growth still requires one normalization of accumulated descriptors; a general dependency-aware incremental resolver is outside this bounded change. Stable snapshots share private immutable normalized collections; mutation is restricted to snapshot-specific fields, criteria and roles. DbNames and schema remain value-owned to preserve existing const accessors. An allocation counter measured over one gigabyte of deep-clone requests across 36 UNF growing finalizations before sharing; the candidate counter quantifies the remaining value-owned copies.

The Debug representations fed into snapshot_fingerprint and its resulting fingerprint bytes remain unchanged. The whole private snapshot Debug layout is not a compatibility promise. Runtime facts do not expose a query fingerprint; finalization is explicit. No measurements of new 1C semantics are needed because behavior is unchanged.

## Descriptor ordering

A reliable frame-pointer profile attributed repeated large memmove calls to sorting ConfigDescriptor values in finish_config_metadata. The implementation sorts source indices by the exact existing resource GUID, object GUID and name tuple, breaking ties by original position; the inverse permutation then moves descriptors through at most linear swaps. This is an internal representation optimization preserving stable order, not a new platform rule. Tests compare all rotations of duplicate-key inputs with the old stable sort.

The direct legacy complete resolver skips transient runtime separator-field construction and delegates to the shared complete finalizer. Runtime-only results still construct separator facts before publication.
