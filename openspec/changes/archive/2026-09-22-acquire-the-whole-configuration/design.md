# Design — acquire the whole configuration in one read

## Context

Two consumers exist for what acquisition already reads. The query
compiler wants names; `open-bsl/bsl-config-db` wants the configuration
resources themselves. Today only the first is served, and the second
would have to re-read `Config` after the transaction ended — against a
base that may have changed in between.

Everything needed is already in place and must be reused rather than
grown a second time: `MetadataSource` with its read-only transaction,
`assemble_parts`, `decode_config_stream`, `ConfigResource`,
`StorageLayout`, `Limits`, the legacy-layout variants, and the
resolution. This change adds reads to that one transaction; it does not
add a pipeline.

## Decisions

### 1. One acquisition, two plans

`acquire_metadata` keeps its signature and becomes a thin wrapper over an
internal `acquire(source, progress, plan)`. The plan differs in exactly
one step:

| step | `ConfigPlan::Names` | `ConfigPlan::Whole` |
| --- | --- | --- |
| `Config` | `read_config` — streams the filtered rows, decodes, keeps nothing | `read_whole_config` — reads every row, assembles, keeps the resources, decodes the metadata-shaped ones from what it kept |
| `_ExtensionsInfo` | not read | `read_extension_catalog` |

Every other step — layout probe, `DBNames`, `ConfigCAS`, the restructure
blobs, `SchemaStorage`, the live catalog, the resolution — is the same
code running in the same order inside the same transaction. The commit
and the rollback stay where they are: the result is returned only after
`commit_readonly` succeeds, and any error goes through
`rollback_readonly`, so no partial result is ever published.

`DatabaseSession::configuration` therefore costs one extra statement pair
over `metadata` (the whole-table read replaces the filtered one, plus
`_ExtensionsInfo`), and `metadata` costs exactly what it costs today.

### 2. The filter lives once, in the core

The filtered statements encode "bare GUID, or GUID with `.1c`, `.9`,
`.7`" as a regular expression (PostgreSQL) and as four `LIKE` character
classes (SQL Server). The whole-table read has no such `WHERE`, so the
same rule has to be applied in Rust to decide which retained resource the
metadata decoder is given.

`open_sdbl::metadata::is_config_metadata_resource(name) -> bool` states
the rule once. `decode_retained_config` — the decoder entry point of the
whole-table plan — consults it and hands the resource decoder only what
it accepts. `decode_config_stream`, which the filtered plan keeps using
unchanged, needs no filter: every name its SQL returned passes anyway.
A test pins the predicate against the shape both statements match.

Progress is reported where the bytes actually arrive. The whole-table
read announces the totals of the whole table and reports each resource as
it is assembled, the ones the predicate rejects included, so the totals
and the advances line up; the decode that follows is silent. The filtered
read keeps reporting at decode time, as it does today.

### 3. Resources are returned as stored

`ConfigResource::compressed` is documented as, and remains, the bytes the
row carries — raw DEFLATE as the platform wrote them, not inflated, not
re-encoded. The consumer inflates what it needs with
`open_sdbl::metadata::inflate_raw_deflate`. Decoding a resource into
`ConfigResource` never copies it twice: the assembled parts are moved
into the returned vector, and the metadata decoder is handed per-batch
clones of the resources it actually decodes, which are dropped as each
batch finishes.

### 4. Extensions are grouped, never flattened

`ConfigCAS` is content-addressed, so a resource row is named by the hash
of its content and says nothing about which extension uses it. Two
extensions that both carry `<guid>.0` for the same role name share one
row when the content is identical and have two rows when it is not;
either way the store cannot be attributed by name.

The attribution is the extension's own root index, which already decodes:
`_ExtensionsInfo` carries the key of the root resource, and
`parse_extension_index` turns that root into `(name, key)` pairs. So the
grouping is:

1. `read_extension_catalog` reads `_ExtensionsInfo`: identity, order,
   name, and the info blob.
2. The whole of `ConfigCAS` is read once — the read acquisition already
   performs, now kept instead of dropped — and indexed by key.
3. For each extension, its root is inflated, its index parsed, and each
   entry resolved against the store into a `ConfigResource` whose
   `file_name` is the **logical** name the index gives (`<guid>`,
   `<guid>.0`) and whose `compressed` is the stored content.

Each extension owns its vector, so equal names across extensions never
merge, and an extension that names a resource the store does not carry
reports that resource as missing rather than borrowing another
extension's.

The grouping is provider-neutral and lives in `pipeline.rs`; the
providers only return rows.

### 5. Identity, order and activity

- **Identity** is `_ExtensionsInfo._IDRRef`, the reference the base gives
  the extension, as lower-case hexadecimal. It is stable across renames,
  which the name is not, and it is what the base itself uses to refer to
  the extension.
- **Order** is `_ExtensionOrder`, the order the platform applies
  extensions in; the statement orders by it and the result preserves that
  order.
- **Activity** is not a column: `_ExtensionsInfo` declares exactly ten
  columns and none of them is a flag. It is carried inside
  `_ExtensionZippedInfo`, which this repository has so far treated as
  opaque. This change decodes that blob into a typed record instead:
  after the four-byte marker and the twenty-byte root key it is a
  tag-length-value stream, in which `0x97` introduces a UTF-16 string of
  *n* code units (the synonym), `0x9a` an ASCII string of *n* bytes (the
  version), and single tagged bytes carry the flags.

  Which flag carries the activity was measured, not guessed. The
  PostgreSQL reference base carries two extensions alike but for the
  platform applying one and not the other; their records are 177 bytes
  each and differ in exactly three places: the twenty bytes of the root
  key, the one character of the synonym that names them apart, and the
  byte three from the end — `0x82` for the applied one, `0x81` for the
  other. Counted structurally that byte is the second-to-last tagged
  flag, which is also where the demo base writes `0x82` for its one
  applied extension even though its record carries a version string and
  the reference base's do not. Both blobs are fixtures, and the test
  asserts the difference between them is that flag.

  A flag that is neither value, or a record with no flags, reads as
  applied: an unfamiliar platform must not silently drop every
  extension.

`_ExtName` stays the name. `_ExtensionUsePurpose` and `_ExtensionScope`
are not part of this change: nothing asked for them, and a field nobody
reads is a field nobody checks.

### 6. `MetadataSource` grows two defaulted methods

`read_whole_config` and `read_extension_catalog` are added with default
bodies. `read_extension_catalog` defaults to "no extensions", which is
what a base without the store has anyway. `read_whole_config` defaults to
a typed error naming the provider, because silently answering an empty
configuration would be a wrong answer rather than a missing feature. Both
providers implement both.

`read_extensions` changes from taking `Vec<ConfigResource>` to taking
`&[ConfigResource]`, so that the store can be kept and grouped after the
call instead of being consumed by it. No implementation overrides it.

### 7. `InfoBaseUser::can_authenticate`

`self.standard_authentication || !self.os_name.trim().is_empty()`, and
nothing else. `AdmRole` is a right, not a way in; `Show` is a list
setting; roles decide what a session may do after it exists, not whether
it can exist. The rule lives on `InfoBaseUser` so that the database
package and the console both ask instead of re-deriving.

## Risks

Holding the whole `Config` table in memory is the point of the operation
and also its cost: a large applied configuration is hundreds of megabytes
compressed. The rustdoc of the operation says so, and `metadata` remains
the read for a caller that only wants names. The decoding limits
(`ConfigDecodeLimits`) still bound what is *inflated* at once; they do not
bound what is retained, because what is retained is what the caller asked
for.

## No new dependency

Nothing here needs one. The statements are text in the core package, the
predicate and the blob decoder are pure functions there, and the reads run
through the drivers the database package already carries.
