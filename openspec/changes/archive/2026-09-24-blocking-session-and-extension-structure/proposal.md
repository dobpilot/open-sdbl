## Why

`open-bsl/bsl-config-db` runs synchronous BSL and has to own one live
connection per session. `open-sdbl-db` already carries `tokio` and
`zeroize`, but it offers only the asynchronous `DatabaseSession`, and the
only way to hand it a password is `EnvironmentSecret::Present(Zeroizing<…>)`.
A synchronous host therefore has to pull both dependencies into its own
tree or write a second executor and a second secrets layer of its own.

The same consumer applies configuration extensions. `AcquiredExtension`
answers identity, name, order, activity and resources, but not what the
base records about an extension beside them — its version, its scope,
whether it is used in a distributed information base — nor which of its
objects are adopted from the main configuration, nor the classes and
objects the extension declares. Guessing any of that from `ConfigCAS`
resource names would be wrong: the store is content-addressed and a name
proves no link.

## What Changes

- **Work A.** `open-sdbl-db` gains `BlockingDatabaseSession`: one value
  owning a single-threaded Tokio runtime and one ordinary
  `DatabaseSession`, with blocking `connect`, `configuration`, `metadata`,
  `query`, `query_each` and `close`. Every method drives the existing
  asynchronous method; there is no second SQL path. Limits, timeouts,
  diagnostics, SQL Server session poisoning and provider behaviour are the
  asynchronous session's. After `close` — and after a failed `close` — every
  call answers the typed `DbError::SessionClosed`. Called from inside an
  asynchronous runtime, a call answers the typed `DbError::AsyncContext`
  instead of panicking. Each value is its own connection.
- **Work A.** `Credentials::new` and `EnvironmentSecret::from_option` take
  owned `String`/`Option<String>` for PostgreSQL, SQL Server and SOCKS5 and
  move them into `Zeroizing` inside the crate, so a consumer never names
  `zeroize`. `Credentials` and `EnvironmentSecret` gain `Debug`
  implementations that never print a secret.
- **Work B.** The core decodes two more structures of an extension: the
  catalog its configuration root declares (classes and the objects of
  each), and the object headers of its descriptors, among them the
  explicit link an adopted object carries to the main-configuration object
  it adopts.
- **Work B.** `_ExtensionsInfo` is read with its `_ExtensionScope` and
  `_UsedInDistributedInfoBase` columns where the base has them; the
  statement answers `NULL` for a column the base lacks.
- **Work B.** `AcquiredExtension` gains `version`, `content_hash`, `safe_mode`,
  `used_in_distributed_infobase`, `scope`, `catalog` and `objects`. Every
  value that is not measured to be decoded reliably is an `Option` and is
  `None` rather than a default; `safe_mode` is `None` on every base today
  (see the design).
- `ExtensionCatalogRow` gains `scope` and `used_in_distributed_infobase`.

The whole-configuration contract (`READ COMMITTED`, no atomic snapshot)
and the retention limits of `Config`/`ConfigCAS` are unchanged. The
fingerprint recommendation of the original request is out of scope of
this change.

## Capabilities

### Modified Capabilities

- `crate-architecture` — a blocking session owner, owned-string
  credentials, structural extension details.
- `onec-metadata` — the extensions statement answers scope and
  distributed-base use; the extension catalog and object headers decode.

## Impact

- `crates/open-sdbl-db`: new `blocking` module, `credentials`, `error`,
  `pipeline`, both providers' extension catalog readers.
- `src/metadata/extension.rs`, `src/metadata/queries.rs`.
- New fixtures under `tests/fixtures/extension/`, taken from the reference
  bases.
- No new dependency: the runtime is `tokio`'s, already a dependency of
  `open-sdbl-db`; `rt` is enabled through the existing `rt-multi-thread`
  feature.
- Breaking for a caller constructing `ExtensionCatalogRow` or exhaustively
  matching `DbError` (already `#[non_exhaustive]`); the workspace version
  moves to 0.9.0.
