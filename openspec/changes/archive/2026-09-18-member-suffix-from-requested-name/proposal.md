## Why

A composite field with a long name — `СубконтоПоАмортизационнойПремии1`
— gets its `_TYPE` label cut to the identifier limit of PostgreSQL, so a
union no longer sees the member and refuses to spread `НЕОПРЕДЕЛЕНО`
of the other branch over it. One accounting corpus query fails so, and
the diagnostic names no field.

## What Changes

- The member a union column carries SHALL be told by the requested
  name, not by the output label the dialect may cut.
- A union width mismatch SHALL name the first field whose width differs.

## Capabilities

### Modified Capabilities

- `query-repl`: member labels survive the identifier limit.
- `query-compilation`: the width diagnostic names the field.

## Impact

`member_suffix` and the width check in
`src/query/core/codegen/orchestrate.rs`.
