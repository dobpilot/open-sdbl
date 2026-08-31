## Context

`ConsoleHelper::from_snapshot` projects fields for every object and again for
every reference field. Candidate insertion uses a vector-wide
case-insensitive search. `queryable_fields` also resolves each `Fld<number>`
name by scanning every resolved field and its owner-table list.

## Decisions

### Add a snapshot-scoped field catalog

Provide a core query helper that derives all currently queryable object fields
in one pass-oriented catalog build. For that build only, record the first
resolved field under every normalized owner table and numeric DBNames field
number, matching the existing scan's source-order behavior. Do not retain the
index in `MetadataSnapshot`, whose public vectors may be changed by callers.

### Cache projections and reference targets per helper build

Project queryable fields at most once for each live named object. Resolve
SchemaStorage reference targets through a normalized physical-table map rather
than rescanning objects per reference field.

### Deduplicate candidates by normalized key

Maintain a lowercase `HashSet` alongside the insertion-order vector. Add the
original candidate only on the first normalized key, then retain the existing
case-insensitive final sort and known-identifier construction.

## Verification

Extend completion tests for duplicate casing, object-qualified fields, and
reference paths. Run all repository gates and measure time to the first prompt
on the reported proxied information base.
