## Context

The console already keeps `\set` parameters in a `ParameterStore`, filters
them to the names a statement references before compiling, and rebuilds
the completion helper after every change. Session parameters and
restrictions follow the same shape: stored for the session, forgotten by
nothing but an explicit command, applied per query without triggering the
compiler's "supplied but unused" diagnostics.

## Decisions

### Session parameters

`ParameterStore` is reused as the session store; `session_parameters()`
converts every entry into a `SessionParameters` value, so no filtering by
reference is needed (the compiler exempts session parameters from the
unused check). Commands:

| Command | Effect |
|---|---|
| `\session Имя литерал` or `\session Имя = литерал` | store or replace |
| `\session` | list name, literal as entered, kind |
| `\session clear` | forget all |

A query parameter set with `\set` and referenced by the statement wins over
a session parameter of the same name, as the library specifies.

### Restrictions

New `RestrictionStore` in `restrict.rs`, one entry per target: the name as
typed, the resolved `ObjectId`, the optional tabular-section name, and the
condition text. `\restrict <имя> <условие>` resolves the name through
`find_metadata_object` at once (so a typo fails immediately) and splits a
third dotted segment off as the section name; the condition itself is not
validated until a query uses it, because validation needs the dialect and
the session values. A second `\restrict` for the same target replaces the
first. `\restrict` lists entries, `\restrict clear` forgets them.

Per query: after `prepare_with`, `RestrictionStore::for_request` returns
`AccessRestriction`s for the requested targets only; those and the session
parameters go into `CompileOptions`. Diagnostics of kind `Restriction`
print like every other compiler diagnostic.

### Refresh

`\refresh` keeps session parameters and restriction text but re-resolves
nothing: restriction entries store the `ObjectId`, which survives a
metadata reload as long as the object exists; a vanished object simply
never matches a request again.

## Risks / Trade-offs

- Restriction text errors surface only at query time, positioned inside the
  restriction text; the message names the target so the user knows which
  `\restrict` to fix.
