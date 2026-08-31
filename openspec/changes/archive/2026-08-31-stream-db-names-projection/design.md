## Context

`parse_db_names` currently inflates the resource, calls `parse_serialized` to
allocate every list and scalar, recursively searches that tree for three-value
records, then drops the entire tree. Live profiling attributes most remaining
startup CPU and allocator activity to this path.

## Decisions

### Use a DBNames-specific streaming projection

Implement the same brace grammar as the generic parser with a recursive cursor.
Each list retains at most its first three lightweight scalar candidates and a
value count. Nested lists are parsed recursively and can emit entries of their
own. On list close, exactly three candidates are projected only when they match
GUID atom/string, quoted alias, and numeric atom semantics used today.

### Validate all input, including irrelevant branches

The streaming parser still checks UTF-8, BOM, separators, list termination,
quoted-string escaping, atoms, whitespace, and trailing input. Unsupported
structures are ignored only after being parsed successfully; malformed
irrelevant branches remain fatal.

### Borrow scalars until an entry is accepted

Atoms and strings without doubled quotes borrow slices from the decoded input.
Only escaped strings allocate during parsing, and only accepted aliases and
canonical GUIDs become owned result data. At most three candidates are retained
per recursion level.

## Verification

Compare streaming output with the generic projection on representative nested
and escaped fixtures, retain malformed-input tests, then repeat the reported
live startup timing and profile.
