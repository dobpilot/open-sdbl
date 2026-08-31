## Context

The resolver currently nests all DBNames `Fld` entries over every
SchemaStorage table/column and every live table/column. On the reported base,
the resulting string comparisons dominate startup after resource parsing is
optimized.

## Decisions

### Index field membership in source order

Traverse SchemaStorage once in table order and map each canonical `Fld<number>`
column base to the owning table's canonical physical name. Deduplicate a field
within one table while retaining table order, matching the previous per-field
scan.

Traverse live catalog columns once, recase each identifier once, and retain the
canonical `_Fld<number>` bases that meet the existing exact-or-compound-suffix
rule.

### Index live tables by normalized physical name

Build a normalized-name-to-table-position map and use it for object live-state,
allowed-length inference, and standard-field indexing. Keep the first matching
table to match iterator `find` semantics if malformed duplicate input is
supplied.

## Verification

Add parity coverage against a scan-based reference fixture, run repository
quality gates, and measure the reported console startup through SOCKS5.
