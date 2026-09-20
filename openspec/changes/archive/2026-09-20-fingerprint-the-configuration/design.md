# Design — fingerprinting a configuration

## Context

`PostgresMetadataQueries` and `MsSqlMetadataQueries` hold every SELECT the
adapters send. `CONFIG_TOTALS` already aggregates over exactly the
resources the acquisition reads, matched by the same GUID-shaped
`FileName` patterns, and it exists so that progress can be reported before
the data is fetched. The fingerprint is the same shape with a stronger
summary.

## Decisions

### 1. Hash per resource, summed over resources

The statement must be one round trip, must not move the data, and must not
depend on row order — a server is free to return aggregate input in any
order, and neither provider guarantees one without a sort the planner is
allowed to skip.

So: hash each resource's bytes, reduce the hashes with an
order-independent combiner.

- PostgreSQL: `md5(binarydata)` per row, read as a number and summed. The
  digest is taken of the bytes, so a rewrite of the same length changes
  it; the sum is order-independent, so the planner may return rows however
  it likes. The count and the byte total travel with it, so a collision in
  the sum alone is not enough to look unchanged.
- SQL Server: `HASHBYTES('SHA2_256', …)` per row, likewise folded into a
  sum, with the same count and byte total beside it.

`md5` here is a change detector, not a security primitive: it answers
"did this resource change", and an adversary who can rewrite `Config`
already owns the configuration. SQL Server gets SHA-256 because
`HASHBYTES` offers it at the same cost.

### 2. Parts are hashed as parts

A modern base splits a resource across rows with a part number. The
statement hashes each row and sums, so it does not have to reassemble
anything — a change in any part changes that part's hash and therefore the
sum. The legacy layout is the same statement with one row per resource,
which is why one statement serves both and no `StorageLayout` variant is
needed.

### 3. The snapshot fingerprint is a different question

`SnapshotFingerprint` answers "is this the snapshot I compiled against",
which is what `Prepared` already uses it for. It cannot answer "has the
base changed", because computing it requires the load the caller is trying
to avoid. Both become public with that distinction stated, so the two are
not confused.

## Risks

- A sum of digests is weaker than a digest of the sorted digests. The
  trade is deliberate: ordering the input would force a sort over every
  resource of the configuration, which is the cost this statement exists
  to avoid. The count and the byte total accompany the sum, so a change
  has to collide in three aggregates at once to pass unnoticed.

## No new dependencies

The statements are text in the core crate; running them belongs to the
application, as with every other acquisition statement.
