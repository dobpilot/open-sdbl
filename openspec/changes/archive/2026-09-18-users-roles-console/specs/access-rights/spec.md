## ADDED Requirements

### Requirement: Rights not listed follow the role default
A rights resource records only what differs from the role's default: a
right the resource does not list for an object, and every right of an
object it does not list at all, SHALL count as granted when
`setForNewObjects` holds and as refused otherwise. `RoleRights::grants`
SHALL answer so, and `read_access` SHALL use it, so that `ПолныеПрава`
— which lists nothing but its refusals — grants `Чтение` of every table.

#### Scenario: Full rights role
- **WHEN** `ПолныеПрава` (УНФ) lists only refused interactive deletions
  for `Справочник.Организации`
- **THEN** it grants `Чтение` of the catalog and refuses
  `ИнтерактивноеУдаление`

#### Scenario: Read-only role
- **WHEN** a role without `setForNewObjects` does not list an object
- **THEN** it grants no right on it
