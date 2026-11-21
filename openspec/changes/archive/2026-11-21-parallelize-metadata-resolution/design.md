# Design

`resolve_fields` first builds the same schema, live-table, descriptor and reference-type indexes as before. It then partitions the filtered DBNames entries into ordered chunks. Each scoped worker resolves its chunk independently against read-only indexes and returns one field plus an optional separator finding per entry.

The parent joins chunk handles in input order. Fields and findings therefore have exactly the sequential order regardless of worker completion order. A worker is assigned at least 256 entries and worker count never exceeds `available_parallelism`; small inputs take the existing synchronous path. No shared mutable state, database handles or async runtime work crosses into the workers.

After fields and index comparisons are ready, report construction, metadata-index construction and the existing fingerprint calculation run as three independent scoped jobs for inputs of at least 256 fields. The fingerprint algorithm and its byte sequence remain unchanged. Smaller inputs execute all three jobs synchronously.

The existing `run_metadata_blocking` call remains the outer boundary, so CPU-heavy resolution does not move onto Tokio runtime workers. Resolution output, reports, access checks and fingerprint inputs are unchanged.
