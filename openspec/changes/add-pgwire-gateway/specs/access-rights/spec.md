## ADDED Requirements

### Requirement: Verify a password against the stored hash
`UserData::verify_password` SHALL answer whether a password matches the
hash the decoded `v8users` record stores for standard 1C authentication,
using the scheme measured on probe bases of both storage layouts and
implemented in the core without dependencies. It SHALL answer false when
the record carries no readable hash. Whether the user may log in at all
SHALL remain the answer of `InfoBaseUser`.

#### Scenario: Known password on a probe base
- **WHEN** the record of a probe-base user created with a known password is
  decoded and checked with that password
- **THEN** the answer is true, and false for the same password with one
  character changed

#### Scenario: Empty password
- **WHEN** a user was created with an empty password
- **THEN** the empty string verifies and any non-empty string does not

### Requirement: Keep password hashes private
The stored password hashes SHALL NOT be exposed through any public item or
`Debug` output, and the comparison SHALL take constant time with respect
to the stored hash.

#### Scenario: Hashes stay private
- **WHEN** a decoded `UserData` is formatted with `Debug`
- **THEN** the output contains no password hash
