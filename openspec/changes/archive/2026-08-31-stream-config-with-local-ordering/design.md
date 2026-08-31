## Context

The current query performs a sequential scan followed by a full sort before
streaming 50,737 Config rows. The decoder already runs in bounded batches, but
the server sort delays row delivery and prevents scan/decode overlap.

## Decisions

### Sort decoded resource groups locally

Each blocking batch returns one descriptor group per input resource together
with its original filename. After the bounded stream completes, sort groups by
filename and flatten them into the resolver input. This frees compressed row
data continuously and preserves descriptor source order within a resource.

### Preserve bounded acquisition

Do not collect all compressed Config rows for sorting. Only decoded descriptor
groups and their filenames survive until final ordering; database delivery and
batch decoding remain bounded by the existing pipeline depth.

## Verification

Cover local ordering across reversed resource input and multiple descriptors,
compare ordered and unordered SQL with `EXPLAIN ANALYZE`, and A/B test release
startup through the reported SOCKS5 connection.
