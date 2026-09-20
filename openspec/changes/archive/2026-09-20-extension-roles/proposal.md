## Why

A user may hold a role of a configuration extension. Its rights are not
in `Config`: the extensions keep their resources in `ConfigCAS`, a
content-addressed store where the name of a row is the hash of what it
holds. The console cannot read such a role at all — it reports that the
role grants nothing — and the demo base of «1С:Документооборот» has one.

The chain is measurable: `_ExtensionsInfo.ExtensionZippedInfo` carries,
after a four-byte marker, the twenty-byte key of the extension's root
resource; that resource lists every resource of the extension by name
with the key of its content, base64-encoded; and the resource
`<guid>.0` of a role is the same rights format `Config` holds.

## What Changes

- The library SHALL read the root key of an extension and the index its
  root carries, and SHALL parse a resource that holds several records
  one after another, as that root does.
- The library SHALL provide statements reading `_ExtensionsInfo` and one
  resource of the extension store by its key, for both providers.
- The console SHALL read the rights of a role the extensions declare and
  name it by its descriptor, so a user holding it takes its access.

## Capabilities

### Modified Capabilities

- `onec-metadata`: the resource index of a configuration extension.
- `query-repl`: roles of an extension.

## Impact

`src/metadata/value.rs`, `src/metadata/extension.rs`,
`src/metadata/queries.rs`, `src/metadata/roles.rs`,
`crates/open-sdbl-cli/src/access.rs`.
