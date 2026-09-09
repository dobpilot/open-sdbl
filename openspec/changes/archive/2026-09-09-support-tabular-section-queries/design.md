## Context

The parser currently stores each projection directly and therefore has nowhere
to retain an output alias. `SourceAst` stores a metadata kind and object plus
register virtual-table variants, but not a document or catalog tabular section.
The required authoritative relationships are already present in the snapshot:

- a tabular-section descriptor has the parent object's GUID as
  `resource_guid` and its own GUID as `object_guid`;
- DBNames assigns that section GUID an exact `VT` number;
- SchemaStorage serializes a section as an inline table
  `{"VT<number>","I",0,"<parent>",...}`, while the live catalog exposes
  `<parent-physical-table>_VT<number>` and its columns.

## Decisions

### Retain aliases on projection items

Wrap each parsed projection with an optional explicit alias token. `КАК` and
`AS` require a following contextual identifier. SQL generation uses that alias
as the stable logical result label. A compound projection retains a deterministic
physical-member suffix under the requested alias rather than collapsing several
SQL columns to the same name.

### Resolve tabular sections through GUID relationships

After `<kind>.<object>.`, known register functions followed by parentheses keep
their current meaning. Any other contextual identifier is parsed as a tabular
section name. Resolution first resolves the parent object normally, then finds
one descriptor whose `resource_guid` equals the parent GUID and whose name
matches the section. The descriptor's `object_guid` must have exactly one
DBNames entry with alias `VT`. The canonical physical table is constructed as
`<parent-physical-table>_VT<number>`. SchemaStorage and the live catalog must
contain either that canonical table or one or more of its exact
configuration-extension `X[digits]` variants. The variants are read through
the same deterministic `UNION ALL` relation already used for extended main
objects.

No general physical prefix scan or numeric-name inference is used: only the
canonical name and its exact `X[digits]` variants qualify. Duplicate descriptor
or DBNames matches are reported as ambiguity.

### Canonicalize SchemaStorage inline tables

The SchemaStorage decoder recognizes the verified inline-table signature
`{"VT<number>","I",0,"<parent>",...}`. It projects the declaration as a
normal `SchemaTable` named `<parent>_VT<number>` so downstream metadata indexes
continue using canonical physical names. Because SchemaStorage encodes the
owner relationship in the table header rather than in its counted column list,
the projection adds the implied `<parent>_IDRRef` reference column with
`<parent>` as its authoritative target. Unknown `I` declarations and malformed
VT numbers or parents are not projected.

### Normalize only platform standard section fields

Custom fields continue to obtain names by their exact SchemaStorage owner table.
For a tabular section, the physical reference column whose SchemaStorage target
is the parent table is additionally exposed as `ID`/`Ссылка`, and the exact
`LineNo<number>` column is exposed as `LineNo`/`НомерСтроки`. Other platform or
unknown columns remain available under their canonical schema names.

### Reuse ordinary source and JOIN compilation

Once resolved, a tabular section supplies the same live table and
`QueryableField` list as an ordinary source. Existing JOIN equality validation,
one-hop dereference joins, filtering, ordering, dialect conversion, and limit
handling remain shared. Presentation of the section owner reference must join
the parent table; it must not treat the section row as the complete parent row.

When a JOIN equality compares a compound multi-target reference with a fixed
reference, the compiler compares their `RRef` payload columns and constrains
the compound `RTRef` member to the fixed reference target's authoritative
metadata number. Comparing only UUID bytes would permit a false match across
different 1C reference types. An absent or ambiguous fixed target or database
type number is diagnosed instead of weakening the predicate.
PostgreSQL emits that four-byte discriminator through `decode('<hex>', 'hex')`
so the SQL remains valid for 1C databases whose session setting keeps
`standard_conforming_strings` disabled.

## Risks / Trade-offs

- Only document and catalog tabular sections are enabled initially because
  their owner-source semantics are covered by the reported query shape.
- Table-part completion is not part of this change.
- The section name depends on a retained nested Config descriptor; a damaged or
  incomplete Config resource fails explicitly instead of falling back to a
  physical table guess.
