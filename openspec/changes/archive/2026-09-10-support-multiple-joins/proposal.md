## Why

A branch may contain exactly one join today. Reports routinely chain
several: a document header, its tabular section, and two catalogs. Each
extra join now forces the user to split the query or fall back to
dereference paths that only cover one hop.

## What Changes

- Replace the single optional join with an ordered list of joins per branch;
  each join introduces one more source scope.
- Let every `ПО` condition reference the newly joined source and any earlier
  source, keeping the existing anchor rule (at least one top-level direct-field
  equality between the new source and an earlier one, plus scalar predicates
  under `И`).
- Keep `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` restricted to the only join of a branch;
  any other join in the same branch is a positional diagnostic.
- Emit dereference and presentation joins after the native join chain, as
  today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: several joins per branch.

## Impact

- `SelectAst::join: Option<JoinAst>` becomes `joins: Vec<JoinAst>`; the
  compilation context already models sources as scopes.
- Existing single-join goldens are unchanged.
