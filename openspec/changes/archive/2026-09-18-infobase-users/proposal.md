## Why

The users of an information base live in the table `v8users`; the roles
of a user are inside its `Data` column, a repeating-key XOR blob whose
key is its own prefix and whose content is a brace-serialized record.
Nothing reads it today, so the roles of a user cannot be mapped to the
rights the library now decodes.

## What Changes

- `decode_user_data` SHALL decode the `Data` blob into the user
  identifier, name, full name and role identifiers; the password hashes
  it carries SHALL NOT be exposed.
- `InfoBaseUser` SHALL combine the columns of `v8users` with the decoded
  data.
- The acquisition statements SHALL read `v8users` on both providers.

## Capabilities

### Modified Capabilities

- `access-rights`: information-base users and their roles.
- `onec-metadata`: the users acquisition statement.

## Impact

`src/metadata/users.rs` (new), `queries.rs`.
