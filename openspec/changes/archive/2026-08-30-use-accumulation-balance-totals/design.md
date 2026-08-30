# Design: DBNames-resolved balance totals

The compiler resolves the totals table from the accumulation-register object's
GUID and its `DBNames` entry whose alias is exactly `AccumRgT`. It constructs
the canonical `_AccumRgT<number>` name, requires the table in SchemaStorage and
the live PostgreSQL catalog, and verifies Period, every Config dimension/data
separator, and every Config resource against the live totals columns. No table
prefix scan or adjacency heuristic is permitted.

Without a period argument, Balance reads only rows at the maximum totals
period. It applies the virtual condition to totals dimensions, sums resources
across `_Splitter` rows by logical dimensions, and removes all-zero groups.

With an exclusive boundary, a scalar anchor selects the greatest stored totals
period not after the boundary; if none exists, it selects the latest stored
period. A totals branch contributes the stored signed resource values. A
movement branch contributes active signed movements between the anchor and
boundary: it adds movements when the anchor precedes the boundary and subtracts
them when a later current-total anchor is used. The two branches are combined
with `UNION ALL` and grouped once. This keeps the totals table authoritative
while supporting a database that has only current totals.

The virtual condition is compiled independently for totals and movement
aliases and is limited to direct dimensions/data separators as before. An
outer WHERE stays outside the final grouped relation. Turnovers is unchanged
and continues to aggregate movement rows.
