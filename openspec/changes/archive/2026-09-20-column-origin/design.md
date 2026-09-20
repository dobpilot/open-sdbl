# Design — the origin of a result column

## Context

Three facts already exist inside the compiler and meet at the column
assembly in `codegen/select.rs`:

- `SourceScope` holds the `ObjectId` of the source and its
  `QueryableField`s;
- `ResolvedPath` holds the scope, the owner `ObjectId` the path ended on,
  and the index of the field within that owner's fields;
- `projected_members` decides how many result columns one field becomes.

The assembly uses the SQL text, the label and the kind, and drops the
rest.

## Decisions

### 1. The identity travels with the field, not looked up by name

A column could ask `snapshot.field_id(owner, name)` at assembly time, but
that repeats a name resolution that has already happened and can answer
`AmbiguousField` for a name two fields share — attaching the wrong origin,
or none, exactly where a mask matters most.

So `QueryableField` gains `field: Option<FieldId>`, filled where the field
is projected. `index_custom_field_names` already walks `snapshot.fields()`
to name custom fields by `(owner table, number)`; the same walk carries
`MetadataField::guid`, which is the `AttributeId`. A standard field takes
`StandardFieldId` from its canonical schema name. A column neither
resolves — a service column, an extension column with no descriptor —
stays `None`.

This also means the origin is exact for a dereference: the path ends on
the target's fields, so the field identity is the target's.

### 2. The shape of the origin

```rust
pub struct ColumnOrigin {
    pub object: ObjectId,
    pub table_part: Option<String>,
    pub field: FieldId,
    pub composite_member: bool,
}
```

`table_part` needs `SourceScope` to carry it; the resolved source already
computes it for the restriction target, so the scope just keeps it.

`composite_member` is true when `projected_members` turned one field into
more than one column, which is how a consumer knows that hiding the
attribute means hiding several columns.

`CompiledColumn::origin` is `Option<ColumnOrigin>`: `None` is the honest
answer for an expression, and makes the addition compatible with callers
that construct nothing.

### 3. Nested sections

`NestedResult::columns` are `CompiledColumn`s built by the same assembly,
so they gain origins by the same path. Their `table_part` is the section,
and their object is the section's owner, which is what the restriction
target already says for a section read as a source — the two agree.

### 4. What does not change

No SQL changes: the origin is metadata attached to the column description,
computed from values the compiler already had. Labels keep their
truncation and de-duplication, and the origin is independent of both
because it never passes through a label.

## No new dependencies

Confined to the core crate.
