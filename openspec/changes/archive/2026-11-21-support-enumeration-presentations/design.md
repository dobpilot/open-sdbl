# Design

Enumeration values are identified by metadata GUIDs; their physical reference
bytes are the 1C byte-order encoding already used by `VALUE`. The value's
Config descriptor carries its owner GUID, name and localized synonyms. Build
the Russian presentation from the nonblank `ru` synonym or the value name.
Never infer a value by order or use a synonym from a different owner.

Extend the structured presentation expression with an enumeration mapping
whose entries contain physical GUID bytes and presentation text. The compiler
validates the target kind, reference field, duplicate identifiers and a bounded
entry count, then emits a dialect-quoted CASE over the target reference column.
The generated literals remain data, never raw SQL. Unknown IDs map to empty
text, as do null/empty references. The same plan is used for ordinary joined
and deferred lookup SQL, which retains its existing access decisions and data
separator filtering. The metadata and policy remain in existing library layers.
Source-free enumeration value presentations return the existing typed deferred
payload and therefore use the same authorized lookup. The new public enum
variant requires downstream exhaustive matches to be updated.

The gateway describes a visible top-level column with one statically known
enumeration target as PostgreSQL `text` and obtains its bounded value mapping
through the same authorized batched lookup before streaming the main results.
This avoids imposing a full-result buffer limit on ordinary enum projections.
Main SQL keeps raw
references for DISTINCT, UUID, type, predicates and internal joins. Other
references and ambiguous runtime types keep their existing binary wire type.
Null stays null and empty or unknown references present as empty text. Simple
and extended query descriptions, text and binary result formats, result
buffering, and the virtual catalog must agree on the text type. The gateway
keeps separate database row indexes for hidden service columns. A denied target
never exposes its presentation. Lookup compilation retains the prepared query's
snapshot mismatch check.

Regression tests use synthetic enumeration metadata with distinct GUIDs,
synonyms and difficult quoting, and verify PostgreSQL and MSSQL SQL, deferred
lookup, missing synonyms, invalid mappings, and existing presentation behavior.
No additional production dependency is needed.
