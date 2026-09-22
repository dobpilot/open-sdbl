## Why

`DatabaseSession::metadata` reads `Config` through a filter — bare GUID
names and their `.1c`, `.9` and `.7` companions — decodes what it needs
for name resolution, and drops every byte. A consumer that wants the
configuration itself, not only the names the query compiler needs, has no
way to get it from the session it already opened.

Reading `Config` again afterwards is not an answer. Between the two reads
the base can be written to: a configuration can be updated, an extension
enabled, a resource rewritten. The snapshot the consumer resolved against
and the resources it then loads would describe two different
configurations, and nothing in the result would say so.

The extension side has the same gap. `ConfigCAS` is content-addressed:
the whole store is read during acquisition and thrown away, and the
resources of two extensions that carry the same name are indistinguishable
once flattened. A consumer that must apply extensions needs them kept
apart, in the order the platform applies them.

Separately, `InfoBaseUser` carries `standard_authentication` and `os_name`
but answers no question about them. Every caller that wants "can this user
log in at all" writes the rule again, and `AdmRole` invites writing it
wrongly.

## What Changes

- `DatabaseSession` SHALL gain one operation that answers the resolved
  metadata, the resolution report, the storage layout, **every** resource
  of the `Config` table, and the resources of each configuration
  extension — all read in the one read-only transaction the metadata
  acquisition already opens.
- The result SHALL be published only after the whole acquisition
  succeeds; any failure SHALL roll the transaction back and answer the
  error, never a partial result.
- The same resources that are returned SHALL be the ones name resolution
  decodes. There SHALL NOT be a second metadata pipeline, nor a second
  path to the database.
- Each acquired extension SHALL keep its identity, its name, whether the
  base applies it, its application order, and its own resources, so that a
  consumer can apply only the active extensions in the platform's order.
- `DatabaseSession::metadata` SHALL keep its behavior and its cost: it
  SHALL NOT begin to retain the whole configuration.
- `InfoBaseUser` SHALL answer whether the user has any way to
  authenticate.

## Capabilities

### Modified Capabilities

- `onec-metadata`: whole-`Config` acquisition statements, the extension
  catalog statement, and the resource name filter the core owns.
- `crate-architecture`: the combined acquisition operation of the
  database package and what it re-exports.
- `access-rights`: `InfoBaseUser::can_authenticate`.

## Impact

- `src/metadata/queries.rs` — whole-table `Config` statements and totals
  for both providers, an extended `_ExtensionsInfo` statement, both
  `all()` lists.
- `src/metadata/config.rs` — the resource-name predicate the filtered
  statements encode, so one definition serves SQL and Rust.
- `src/metadata/extension.rs` — the typed record of
  `_ExtensionZippedInfo`.
- `src/metadata/users.rs` — `InfoBaseUser::can_authenticate`.
- `crates/open-sdbl-db/src/pipeline.rs` — one acquisition driving both
  operations, the acquired-configuration types, extension grouping.
- `crates/open-sdbl-db/src/db/{postgres,mssql}/metadata.rs` — the
  provider reads behind the new `MetadataSource` methods.
- `crates/open-sdbl-db/src/{session.rs,lib.rs}` and both provider
  sessions — the operation and the re-exports.
- No new production dependency. The core package stays I/O-free: it
  contributes statements, predicates and decoders only.
