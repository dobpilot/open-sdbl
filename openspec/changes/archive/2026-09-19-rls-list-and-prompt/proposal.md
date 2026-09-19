## Why

`\restrict` alone lists what it stores, `\rls` alone only prints its
usage — there is no way to see which objects the loaded roles restrict.
And with a current user set by `\as`, nothing in the console says so:
the prompt stays `open-sdbl=>`, so a `РАЗРЕШЕННЫЕ` query silently
carries that user's restrictions.

## What Changes

- `\rls` without an object SHALL list the restrictions of the rights
  already read — role, object and right — instead of reporting its
  usage.
- The prompt SHALL name the current user while `\as` holds one, and
  return to `open-sdbl=>` on `\as clear` and on `\refresh`.

## Capabilities

### Modified Capabilities

- `query-repl`: `\rls` alone lists the loaded restrictions; the prompt
  names the current user.

## Impact

`crates/open-sdbl-cli/src/access.rs`, `crates/open-sdbl-cli/src/repl/mod.rs`.
