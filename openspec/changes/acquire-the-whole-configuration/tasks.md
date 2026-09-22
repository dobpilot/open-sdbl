## 1. Statements and predicates in the core

- [x] 1.1 `PostgresMetadataQueries` and `MsSqlMetadataQueries` gain the
  whole-table `Config` statements in both layout variants, their totals
  statement, and the selector picking the variant for a `StorageLayout`.
- [x] 1.2 Both `all()` lists carry the new statements.
- [x] 1.3 `EXTENSIONS` answers identity, order, name and the info blob,
  ordered by `_ExtensionOrder`; its one existing reader follows.
- [x] 1.4 `is_config_metadata_resource` states the acquisition name
  filter once, and a test pins it against the statement shapes.

## 2. The extension record

- [x] 2.1 `_ExtensionZippedInfo` decodes into a typed record: root key,
  synonym, version and flags, answering the root key even when the tail
  is unfamiliar.
- [x] 2.2 The active flag is identified by measurement against a base
  carrying one extension enabled and disabled, and pinned by a fixture of
  both blobs.
- [x] 2.3 `extension_root_key` keeps working for its existing callers.

## 3. The acquisition

- [x] 3.1 `acquire_metadata` becomes a wrapper over one internal
  acquisition taking the plan; the order of every other step is
  unchanged.
- [x] 3.2 `MetadataSource::read_whole_config` and
  `read_extension_catalog`, both defaulted, both implemented by both
  providers; `read_extensions` takes a slice.
- [x] 3.3 `decode_retained_config` decodes only the resources the
  predicate accepts; the whole-table read counts the rest towards
  progress as it assembles them.
- [x] 3.4 `acquire_configuration` answers the acquired configuration,
  grouping extension resources through each extension's root index.
- [x] 3.5 `DatabaseSession::configuration` and both provider sessions;
  `metadata` untouched.
- [x] 3.6 `ConfigResource`, the acquired-configuration type and the
  acquired-extension type re-exported from the package root, with the
  byte contract documented.

## 4. `InfoBaseUser::can_authenticate`

- [x] 4.1 The method, documented, owning the rule.

## 5. Tests

- [x] 5.1 A deterministic in-memory `MetadataSource` over fixtures,
  driving both plans.
- [x] 5.2 The whole read answers the same snapshot and report as the
  metadata read, with the resources added.
- [x] 5.3 The answered resources include one the acquisition filter
  rejects.
- [x] 5.4 A resource stored in parts is answered once, assembled.
- [x] 5.5 A gap, a repeat and a late start each fail naming the resource.
- [x] 5.6 A failing read rolls back and answers no configuration.
- [x] 5.7 Two extensions keep their identity, order and resources apart.
- [x] 5.8 Equal resource names across two extensions do not merge.
- [x] 5.9 An inactive extension is answered as inactive.
- [x] 5.10 `can_authenticate` over the five cases, the last one across
  every combination of the two flags that do not decide.
- [x] 5.11 Both providers' statements are SELECT-only, cover both
  layouts, and are listed in `all()`.
- [x] 5.12 The existing metadata tests pass unchanged.

## 6. Review findings

- [x] 6.1 The consistency claim is narrowed everywhere it was made —
  rustdoc, README, spec — and the fingerprint statement is named as what
  a consumer compares instead.
- [x] 6.2 The extension record answers `Option<bool>`, never a flag read
  out of a payload: the walk knows its tags, stops at anything else, and
  trusts the byte only when it reached the terminator and read that byte
  as a standalone tag.
- [x] 6.3 `parse_extension_info` answers `None` only without the root
  key; a field that ends early answers the key and what preceded it.
- [x] 6.4 `Limits` bounds retained resources and retained bytes; the
  totals are checked before the first row and the ceiling again as rows
  arrive.
- [x] 6.5 `ConfigResource::compressed` is `Arc<[u8]>`, so shared store
  rows and decoder batches cost a reference, not a copy.
- [x] 6.6 Tests: unknown tag, unknown tag with the terminator, payload
  where the flag sits, a field that ends early, both ceilings before the
  read and during it, and an extension whose activity is unknown.

## 7. Checks

- [x] 7.1 `cargo fmt --all -- --check`
- [x] 7.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 7.3 `cargo test --workspace`
- [x] 7.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 7.5 `cargo tree -p open-sdbl -e normal` still empty
- [x] 7.6 `openspec validate acquire-the-whole-configuration --strict`;
  archive
