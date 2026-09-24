## 1. Contract

- [x] 1.1 `AcquiredExtension::objects` becomes
  `Option<Vec<AcquiredExtensionObject>>`, rustdoc stating when it is `Some`.
- [x] 1.2 The grouping answers `None` when a description resource does not
  inflate or parse, when the catalog is `None`, or when a declared object
  has no header of its own.

## 2. Tests

- [x] 2.1 The measured fixture answers `Some` with every header it
  answered before.
- [x] 2.2 One damaged description resource answers `None`.
- [x] 2.3 A catalog-declared object without its header answers `None`.
- [x] 2.4 A damaged resource whose name carries a suffix leaves the list
  complete.
- [x] 2.5 A catalog that is `None` answers `None`, even with every
  description decoded.

## 3. Close

- [x] 3.1 README; version 0.10.0.
- [x] 3.2 All checks green, `openspec validate --all --strict`; archive.
