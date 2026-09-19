## Why

`\as` applies the restrictions of the user's roles silently, at compile
time: the operator never sees the condition a template expanded to, and
cannot correct it. The expanded condition is ordinary SDBL — the same
language `\restrict` stores — so it belongs in the restriction store,
where it can be listed, overridden and cleared.

## What Changes

- `\as <пользователь>` SHALL expand the `Чтение` restrictions of the
  user's roles and store them in `\restrict` as derived restrictions,
  reporting how many were derived and which could not be expanded.
- A manual `\restrict` SHALL never be overwritten by a derived one, and
  setting one over a derived target SHALL make it manual; `\restrict`
  SHALL mark derived restrictions in its listing.
- `\as clear` and `\refresh` SHALL forget the derived restrictions, and
  a `\session` command SHALL re-derive them while a user is current, so
  a stored condition never outlives the parameters it was expanded with.

## Capabilities

### Modified Capabilities

- `query-repl`: `\as` derives restrictions into `\restrict`; the store
  keeps the origin of each restriction.

## Impact

Expanding a restriction lower-cased the whole remaining text for every
occurrence it searched for, which made one expansion of a БСП template
take about 0.6 s; the search now folds the text once. Deriving the
restrictions of a user of the УНФ demo — 640 roles, 470 restricted
objects — takes 2 s instead of 5 minutes, and every restricted query is
as much faster.


`crates/open-sdbl-cli/src/restrict.rs`,
`crates/open-sdbl-cli/src/access.rs`,
`crates/open-sdbl-cli/src/repl/mod.rs`.
