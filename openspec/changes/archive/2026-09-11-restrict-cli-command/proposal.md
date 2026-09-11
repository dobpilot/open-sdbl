## Why

The library now compiles `РАЗРЕШЕННЫЕ` statements against application
supplied restrictions and session parameters, but the console passes
neither: every restricted query runs unfiltered, and the feature cannot be
exercised end to end on a real base without writing a host application.

## What Changes

- `\session <Имя> [=] <литерал>` stores a session parameter that every
  query and every restriction of the console session sees; `\session`
  lists them, `\session clear` forgets them. Literals follow `\set`.
- `\restrict <Вид.Объект[.ТабличнаяЧасть]> <условие>` stores an access
  restriction for one table; `\restrict` lists them, `\restrict clear`
  forgets them. Before each query the console passes only the restrictions
  whose target the prepared batch requested, so a stored restriction for a
  table the query does not read is never an error.
- With no stored restriction a `РАЗРЕШЕННЫЕ` query runs unfiltered, which is
  the console's "1 = 1" default.
- `&` completion offers session parameter names alongside `\set` ones;
  `\help` and the command completion list gain the new commands.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: console session parameters and access restrictions.

## Impact

- `crates/open-sdbl-cli/src/params.rs` (session store and commands),
  new `crates/open-sdbl-cli/src/restrict.rs`, `repl.rs` (dispatch, help,
  completion, compile options), README command table,
  `docs/query-language-support.md`.
- No new dependencies; the library API from
  `allowed-keyword-restrictions` is consumed as is.
