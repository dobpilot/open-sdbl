## MODIFIED Requirements

### Requirement: Decode the information-base users
The library SHALL decode the `Data` column of `v8users` — a blob whose
first byte is the length of the XOR key that follows it, the rest being
the brace-serialized user record XORed with that key, with a UTF-8
byte-order mark in front — into `UserData`: the user identifier, the
name, the full name and the role identifiers of the `{N, <guid>…}`
block. The password hashes the record carries SHALL NOT be exposed.
`InfoBaseUser` SHALL combine the row of `v8users` — name, description,
operating-system login, e-mail (empty on a platform whose table has no
such column), the show-in-list, standard-authentication and
administrative flags — with the decoded data, and SHALL name the roles
through a `RoleCatalog`.

#### Scenario: A user with three roles
- **WHEN** the `Data` of `Абдулов (директор)` (УНФ demo) is decoded
- **THEN** the name is `Абдулов (директор)`, the full name
  `Абдулов Юрий Владимирович`, and the roles are the three identifiers
  named `АдминистраторСистемы`, `ПолныеПрава` and
  `ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок` by the catalog

#### Scenario: A user without roles
- **WHEN** the `Data` of a user whose role block is `{0}` is decoded
- **THEN** the role list is empty

#### Scenario: A malformed blob
- **WHEN** the blob is shorter than its key length
- **THEN** decoding fails with a `MetadataError`

#### Scenario: E-mail of a user
- **WHEN** the row of a user carries `Email`
- **THEN** `InfoBaseUser::email` is that text, and `\users` and `\user`
  print it
