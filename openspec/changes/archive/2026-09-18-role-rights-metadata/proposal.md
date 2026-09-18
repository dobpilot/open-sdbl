## Why

Row-level security of a 1C base lives in its roles: the `Config` record
`<guid>.0` of every role stores, in the brace format, the rights the
role grants on each metadata object, the restriction conditions
attached to those rights and the restriction templates the conditions
call. Nothing reads them today, so an application embedding the library
cannot tell which restriction a user's roles put on a table.

## What Changes

- A bare-GUID Config resource that carries the roles collection of the
  configuration root (`09736b02-9cac-4e3f-b4f7-d3e9576ab948`) SHALL
  report the role identifiers it lists; `RoleCatalog` names them from
  the descriptors.
- `parse_role_rights` SHALL decode a role's rights resource into
  `RoleRights`: per object, the granted rights with their restriction
  conditions and fields; the restriction templates; the header flags.
- `Right` SHALL name the standard rights by their platform identifiers,
  derived from the order the platform writes them in — measured on
  8.3.27 against БП 3.0 and УНФ — and SHALL keep an unknown identifier
  as `Other`.
- The acquisition statements SHALL read the rights resources of a given
  set of roles only, because a base stores tens of thousands of `.0`
  resources.

## Capabilities

### New Capabilities

- `access-rights`: roles, their rights and restriction texts.

### Modified Capabilities

- `onec-metadata`: the roles collection is projected while parsing; the
  rights acquisition statements.

## Impact

`src/metadata/roles.rs` (new), `ParsedConfigResource.roles`,
`queries.rs`. `ParsedConfigResource` gains a public field, which breaks
a struct literal of it outside the crate (none is known).
