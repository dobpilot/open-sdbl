## 1. Parser and result shape

- [x] 1.1 Retain explicit `КАК`/`AS` aliases for every supported projection
  expression.
- [x] 1.2 Apply aliases to simple and compound output labels in single-source,
  JOIN, UNION, PostgreSQL, and MSSQL compilation paths.
- [x] 1.3 Parse a non-virtual third source segment as a tabular-section name.

## 2. Authoritative tabular-section resolution

- [x] 2.1 Resolve a section descriptor under its parent Config resource and its
  exact DBNames `VT` entry.
- [x] 2.2 Decode SchemaStorage inline `VT` declarations into canonical
  `<parent>_VT<number>` tables with their implied owner reference.
- [x] 2.3 Require the canonical `<parent>_VT<number>` table or its exact
  configuration-extension variants in SchemaStorage and the live catalog,
  project its custom fields, and read all matching variants as one relation.
- [x] 2.4 Normalize the owner reference and line number standard fields without
  inferring unrelated physical names.
- [x] 2.5 Reuse tabular sections in single-source and JOIN compilation and keep
  owner-reference presentation joins correct.
- [x] 2.6 Compile JOIN equality between a compound multi-target reference and
  a fixed reference as an `RRef` comparison plus an authoritative `RTRef`
  discriminator.

## 3. Verification

- [x] 3.1 Cover projection aliases, inline SchemaStorage decoding,
  tabular-section lookup diagnostics, direct reads, JOIN conditions, and
  one-hop dereferences in unit tests.
- [x] 3.2 Reproduce the reported document-tabular-section JOIN against the
  available PostgreSQL information base when connection settings are present.
- [x] 3.3 Update README syntax documentation and run all workspace/OpenSpec
  quality gates.
