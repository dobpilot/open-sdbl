## Why

`v8users` carries an `Email` column on current platforms; `\users` and
`\user` do not show it, and `InfoBaseUser` has no field for it.

## What Changes

- `InfoBaseUser` SHALL carry the e-mail of the user; the console SHALL
  print it in `\users` and `\user`.
- The library SHALL provide a statement probing whether `v8users` has
  the column and a users statement reading it, beside the one that does
  not, so a base of an older platform without the column still answers.

## Capabilities

### Modified Capabilities

- `access-rights`: the e-mail of a user.
- `onec-metadata`: the users statements with the e-mail and the probe.

## Impact

`UserRow.email`, `InfoBaseUser.email`, `USERS_EMAIL_PROBE`,
`USERS_WITH_EMAIL`; `crates/open-sdbl-cli/src/access.rs`.
