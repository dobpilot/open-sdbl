## Why

A role carries the restriction templates its conditions call, and the
console decodes them, but shows only how many there are. An operator who
wants to know why a condition expanded the way it did — or which branch a
session parameter selects — has no way to read the template.

## What Changes

- `\role <имя>` SHALL list the templates of the role by signature.
- A new `\template <роль> [<имя>]` SHALL show the templates of a role with
  their sizes, or the body of one template.

## Capabilities

### Modified Capabilities

- `query-repl`: the console shows the restriction templates of a role.

## Impact

`crates/open-sdbl-cli/src/access.rs`,
`crates/open-sdbl-cli/src/repl/mod.rs`,
`crates/open-sdbl-cli/src/repl/completion.rs`.
