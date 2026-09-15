## Why

`repl.rs` is 2 923 lines and carries at least eight responsibilities: the
read-execute loop, completion, preparing and compiling a statement,
resolving deferred presentations, meta commands, rendering tables,
printing metadata descriptions, and the terminal itself — footer, UTF-8
guard, size detection, bounded line reading.

They change for different reasons and share nothing but the file. A change
to how a table is printed sits next to the code that talks to the server,
and the 25 tests of all eight concerns live at the bottom of the same
file.

## What Changes

- Split `repl.rs` into a directory of focused modules: the loop, then
  completion, statement preparation, presentations, meta commands,
  rendering, metadata description and terminal handling.
- Move the tests of each concern into `src/tests/repl_<concern>.rs`.

## Capabilities

### Modified Capabilities

- `crate-architecture`: a CLI module carries one feature.

## Impact

No behavior change: the same console, the same output, the same
diagnostics. What changes is where a maintainer looks.
