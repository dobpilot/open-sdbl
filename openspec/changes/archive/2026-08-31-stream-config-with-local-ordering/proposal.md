## Why

The Config query orders 50,737 rows on PostgreSQL before the first rows can be
delivered to the asynchronous decoder. `EXPLAIN ANALYZE` shows an explicit
18.4 MiB quicksort and a planner estimate of only 11 rows, so the server cost
is underestimated and database delivery cannot overlap the complete scan and
sort with decoding.

## What Changes

- Remove server-side `ORDER BY filename` from the fixed Config query.
- Retain each decoded resource's filename and restore ascending filename order
  locally after bounded streaming decode.
- Preserve descriptor order within each resource, errors, progress, and the
  bounded database/CPU pipeline.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: define deterministic Config descriptor ordering independently
  of PostgreSQL row-delivery order.

## Impact

- Changes the public `PostgresMetadataQueries::CONFIG` SQL text.
- Adds temporary filename/group bookkeeping after compressed rows are decoded.
- Does not change accepted metadata or resolved snapshot contents and ordering.
