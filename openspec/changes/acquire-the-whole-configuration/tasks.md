## 1. Statements and predicates in the core

- [ ] 1.1 `PostgresMetadataQueries` and `MsSqlMetadataQueries` gain the
  whole-table `Config` statements in both layout variants, their totals
  statement, and the selector picking the variant for a `StorageLayout`.
- [ ] 1.2 Both `all()` lists carry the new statements.
- [ ] 1.3 `EXTENSIONS` answers identity, order, name and the info blob,
  ordered by `_ExtensionOrder`; its one existing reader follows.
- [ ] 1.4 `is_config_metadata_resource` states the acquisition name
  filter once, and a test pins it against the statement shapes.

## 2. The extension record

- [ ] 2.1 `_ExtensionZippedInfo` decodes into a typed record: root key,
  synonym, version and flags, answering the root key even when the tail
  is unfamiliar.
- [ ] 2.2 The active flag is identified by measurement against a base
  carrying one extension enabled and disabled, and pinned by a fixture of
  both blobs.
- [ ] 2.3 `extension_root_key` keeps working for its existing callers.

## 3. The acquisition

- [ ] 3.1 `acquire_metadata` becomes a wrapper over one internal
  acquisition taking the plan; the order of every other step is
  unchanged.
- [ ] 3.2 `MetadataSource::read_whole_config` and
  `read_extension_catalog`, both defaulted, both implemented by both
  providers; `read_extensions` takes a slice.
- [ ] 3.3 `decode_config_stream` decodes only the resources the predicate
  accepts and counts the rest towards progress.
- [ ] 3.4 `acquire_configuration` answers the acquired configuration,
  grouping extension resources through each extension's root index.
- [ ] 3.5 `DatabaseSession::configuration` and both provider sessions;
  `metadata` untouched.
- [ ] 3.6 `ConfigResource`, the acquired-configuration type and the
  acquired-extension type re-exported from the package root, with the
  byte contract documented.

## 4. `InfoBaseUser::can_authenticate`

- [ ] 4.1 The method, documented, owning the rule.

## 5. Tests

- [ ] 5.1 A deterministic in-memory `MetadataSource` over fixtures,
  driving both plans.
- [ ] 5.2 The whole read answers the same snapshot and report as the
  metadata read, with the resources added.
- [ ] 5.3 The answered resources include one the acquisition filter
  rejects.
- [ ] 5.4 A resource stored in parts is answered once, assembled.
- [ ] 5.5 A gap, a repeat and a late start each fail naming the resource.
- [ ] 5.6 A failing read rolls back and answers no configuration.
- [ ] 5.7 Two extensions keep their identity, order and resources apart.
- [ ] 5.8 Equal resource names across two extensions do not merge.
- [ ] 5.9 An inactive extension is answered as inactive.
- [ ] 5.10 `can_authenticate` over the five cases, the last one across
  every combination of the two flags that do not decide.
- [ ] 5.11 Both providers' statements are SELECT-only, cover both
  layouts, and are listed in `all()`.
- [ ] 5.12 The existing metadata tests pass unchanged.

## 6. Checks

- [ ] 6.1 `cargo fmt --all -- --check`
- [ ] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 6.3 `cargo test --workspace`
- [ ] 6.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [ ] 6.5 `cargo tree -p open-sdbl -e normal` still empty
- [ ] 6.6 `openspec validate acquire-the-whole-configuration --strict`;
  archive
