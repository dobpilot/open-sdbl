## Context

`Guid::to_1c_bytes` already converts a canonical UUID `a-b-c-d-e` into the
physical 1C order `d + e + c + b + a`. The query function performs the inverse
inside SQL over a 16-byte reference column.

## Decisions

### Pure SQL permutation into native UUID types

PostgreSQL rebuilds the canonical byte string with `substring` and casts the
hex text to `uuid`:

```sql
encode(substring(c from 13 for 4) || substring(c from 11 for 2)
    || substring(c from 9 for 2) || substring(c from 1 for 8), 'hex')::uuid
```

MSSQL `CAST(binary(16) AS uniqueidentifier)` interprets the first three
groups little-endian, so those groups are reversed byte by byte before the
cast:

```sql
CAST(SUBSTRING(c,16,1)+SUBSTRING(c,15,1)+SUBSTRING(c,14,1)+SUBSTRING(c,13,1)
    +SUBSTRING(c,12,1)+SUBSTRING(c,11,1)+SUBSTRING(c,10,1)+SUBSTRING(c,9,1)
    +SUBSTRING(c,1,8) AS uniqueidentifier)
```

Both forms propagate `NULL` naturally and need no server-side extension.
Known vector: bytes `9022249e3a1ac4b94be8faddd2f8bde9` yield
`d2f8bde9-fadd-4be8-9022-249e3a1ac4b9`.

### Argument shape

The argument is one field reference resolved through the ordinary path
resolver, so one-hop dereferences reuse existing joins. The physical column is
the field's `RRRef`/`IDRRef` member; compound fields therefore decode the
reference part only, and a non-reference value inside such a field decodes to
the zero UUID, matching 1C. Fields without a reference member fail with a
positional syntax diagnostic. Source-free branches and virtual-table period
arguments reject the function as unsupported.

## Risks / Trade-offs

- The function is not available for `ЗНАЧЕНИЕ(...)` constants or binary
  literals; those already have a known GUID on the caller side.
