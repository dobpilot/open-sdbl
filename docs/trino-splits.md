# Trino split strategy

The MVP returns exactly one `ConnectorSplit` for each derived table handle. The
handle contains the resolved object, accepted filters, projected columns, and
minimum limit. The Rust service therefore issues one PostgreSQL statement and
does not claim parallelism it cannot guarantee.

This is deliberate: dividing a table by arbitrary row offsets would duplicate
or omit rows during concurrent writes, while PostgreSQL block ranges and hash
predicates need validation against the 1C storage and its indexes.

A future split descriptor can add an optional, typed partition predicate. Safe
candidates, in preferred order, are:

1. ranges of a declared/live physical primary or unique key;
2. hash buckets over stable UUID/reference payloads;
3. PostgreSQL physical block ranges for snapshot-consistent scans;
4. metadata-aware period/key ranges for registers.

Before enabling multiple splits, the implementation must prove complete,
non-overlapping coverage, retain all pushed predicates and the global limit
semantics, and pin all splits to a compatible PostgreSQL snapshot. Until then,
one split is the correct production-safe behavior.
