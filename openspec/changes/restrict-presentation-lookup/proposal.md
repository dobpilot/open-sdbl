## Why

`RestrictionMode::Restricted` refuses a deferred reference presentation,
because the second query that resolves it —
`compile_presentation_lookup` — reads its target table with no filter, and
producing work for it would mean reading past the decisions of the
compilation that produced the reference.

The refusal is correct and the cost is high. `ПРЕДСТАВЛЕНИЕ(Регистратор)`,
and every presentation of a universal reference, is unavailable in the
mode a gateway always runs in — and asking for the readable name of the
document behind a register record is an ordinary request.

## What Changes

- The presentation lookup SHALL accept access decisions and session
  parameters and apply them to its target table, the way a source of a
  statement is filtered.
- A lookup driven from a restricted prepared query SHALL carry that
  query's mode: a target with no decision SHALL fail, a denied target
  SHALL admit no row.
- With the lookup filtered, `Restricted` SHALL stop refusing a deferred
  reference presentation. It SHALL keep refusing one whose lookup cannot
  be filtered.
- A reference whose target the decision excludes SHALL come back with no
  presentation — not an error, and not the value.
- The existing unrestricted entry point SHALL keep its signature and its
  behaviour, and SHALL stay documented as unfiltered.

## Capabilities

### Modified Capabilities

- `query-compilation`: the deferred presentation lookup under access
  decisions.

## Impact

- `src/query.rs` (a lookup on `Prepared`), `src/query/core/codegen/entry.rs`,
  `src/query/core/codegen/context.rs` (the refusal).
- New tests; no change to an unrestricted lookup's SQL.
- No new production dependency.
