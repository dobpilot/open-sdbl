## Why

A user of a base may hold a role the configuration no longer declares —
a role deleted since, or one belonging to a configuration extension,
whose rights live in `ConfigCas` and are not linked yet. The console
asks `Config` for the rights of every role of the user and fails the
whole command when one resource is absent, so `\as` cannot be used at
all: «the rights resource of role "262144f7-…" is not in Config».

## What Changes

- A role whose rights resource is absent SHALL be remembered as
  unreadable, asked for once, and skipped: it grants nothing.
- `\as` SHALL report how many roles of the user have no rights resource,
  naming them, and SHALL go on with the rest.
- `\role` and `\template`, which name one role, SHALL still report that
  its rights are not in `Config`.

## Capabilities

### Modified Capabilities

- `query-repl`: roles whose rights the base does not carry.

## Impact

`crates/open-sdbl-cli/src/access.rs`.
