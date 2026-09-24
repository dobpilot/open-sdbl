## Why

`AcquiredExtension::objects` is a `Vec`. When a description resource of an
extension does not inflate or does not parse, the reader skips it and
answers the headers of the rest. A consumer applying the extension's
structure cannot tell that list from a complete one, and would run with
objects or attributes of the extension missing — the partial answer the
catalog already refuses to give.

## What Changes

- **BREAKING** `AcquiredExtension::objects` becomes
  `Option<Vec<AcquiredExtensionObject>>`.
- `Some` is answered only for a complete, checked list: every description
  resource of the extension inflated and parsed, and every top-level object
  the catalog declares has a header of its own among them.
- Any failure — a description that does not inflate or parse, a declared
  object without a header, or no catalog to check against — answers `None`
  for the whole list, never a shorter one.
- Nothing is guessed to fill a gap: an adopted object whose header records
  the nil identifier stays an honest header with no link.

## Capabilities

### Modified Capabilities

- `crate-architecture` — the object headers of an acquired extension are
  answered whole or not at all.

## Impact

- `crates/open-sdbl-db/src/pipeline.rs` and its tests; README.
- Breaking for a consumer reading `objects` as a `Vec`; the workspace
  version moves to 0.10.0.
