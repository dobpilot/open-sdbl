## Why

The restriction templates of the Standard Subsystems Library read session
parameters the library itself computes and keeps in the base: the
versions of the templates and the lists whose reading is restricted. An
operator had to type them by hand with `\session`, copying tens of
kilobytes, or watch `\as` report that most restrictions cannot be
expanded.

## What Changes

- The library SHALL read a `ХранилищеЗначения` column: the `STORHDR`
  reference it holds, the parts of the content in `binarydata`, and the
  serialized value they carry, deflated or not.
- `\as` SHALL fill the session parameters the base carries — the five
  the register `ПараметрыОграниченияДоступа` holds and the empty external
  user — leaving alone every parameter the operator typed, and SHALL
  report what it read.

## Capabilities

### Modified Capabilities

- `onec-metadata`: reading a stored value.
- `query-repl`: `\as` reads the parameters of the templates.

## Impact

`src/metadata/value_storage.rs`, `src/metadata/queries.rs`,
`crates/open-sdbl-cli/src/access_cache.rs`,
`crates/open-sdbl-cli/src/access.rs`.
