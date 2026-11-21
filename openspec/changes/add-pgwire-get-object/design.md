# Design

Recognize the standalone call using the same bounded SQL grammar used by
the gateway's other service functions. The stable `jsonb` type lets a client
describe and prepare a statement before supplying the object link.

Accept an internal `e1cib/data/<kind>.<name>?ref=<hex>` object link. Resolve
the object in the current snapshot and use the 16 physical reference bytes
as a typed SDBL parameter. Compile a single wildcard projection filtered by
that reference and limited to two rows so unexpected duplicates can be
detected. Authorization, field checks and forbidden-row validation are
shared with SDBL execution.

Serialize the compiler's logical projection by original attribute name.
For a composite attribute, use its active member. This gives clients one
key per attribute and preserves scalar values without using physical field
columns as keys. Return SQL NULL when the reference has no row. Tabular
sections belong to separate rows and are outside this function's result.
