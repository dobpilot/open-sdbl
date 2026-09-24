## 1. Work A — the blocking session owner

- [x] 1.1 `DbError::SessionClosed` and `DbError::AsyncContext`, with
  predicates, `Display` and no source.
- [x] 1.2 `EnvironmentSecret::from_option`, `Credentials::new`, redacting
  `Debug` for both.
- [x] 1.3 A crate-private operations trait implemented by
  `DatabaseSession` by delegation, and the private `BlockingDriver<S>`
  owning a current-thread runtime and `Option<S>`.
- [x] 1.4 `BlockingDatabaseSession` with `connect`, `configuration`,
  `metadata`, `query`, `query_each`, `close`, `dialect`, `is_dead`,
  `is_closed`; re-exported from the package root.
- [x] 1.5 Tests: equal results and equal errors through the asynchronous
  path and the blocking driver over an artificial provider; two sessions
  are two connections and closing one leaves the other usable; every call
  after close answers `SessionClosed`; a call inside a runtime answers
  `AsyncContext`; the value is `Send`; no password in `Debug` or in the
  error of a refused connection; the blocking module names no statement,
  provider session or driver type.
- [x] 1.6 README and crate documentation.

## 2. Work B — the structure of an extension

- [x] 2.1 Probe and statement for `_ExtensionScope` and
  `_UsedInDistributedInfoBase` for both providers, in the `all()` lists.
- [x] 2.2 `ExtensionCatalogRow` gains `scope` and
  `used_in_distributed_infobase`; both providers read them.
- [x] 2.3 Core: `extension_configuration_root`, `parse_extension_catalog`,
  `parse_extension_objects` with `ExtensionCatalog`, `ExtensionClass`,
  `ExtensionObject`, `ObjectBelonging`.
- [x] 2.4 `AcquiredExtension` gains `version`, `content_hash`, `safe_mode`,
  `used_in_distributed_infobase`, `scope`, `catalog`, `objects`; grouping
  decodes them off the runtime thread.
- [x] 2.5 Fixtures from the demo base: the configuration root of
  `Расширение1`, the descriptions of `СтавкиНДС` and `Расш1_Справочник1`,
  and the info record of a `buh` extension carrying a version.
- [x] 2.6 Tests: identity, order, activity (applied, not applied,
  unknown); every new optional field set and unset; the adopted link
  and the own object; a malformed catalog answering `None`; an unfamiliar
  header tail answering unknown; the same rows through both providers'
  statement shapes answering the same extension.
- [x] 2.7 README and crate documentation.

## 3. Close

- [x] 3.1 All five checks green; `openspec validate --strict`.
- [x] 3.2 Version 0.9.0; archive the change.
