## Why

Information bases created by platform 8.2 (and 8.3 builds before the 8.3.8
storage format) keep `Params`, `Config`, `ConfigSave`, and `Files` without a
`PartNo` column and have no `ConfigCAS` or `_ExtensionsRestruct` tables. The
fixed acquisition queries reference `PartNo` unconditionally, so the console
fails on the first metadata statement (`Invalid column name 'PartNo'`, SQL
Server error 207). Separately, modern bases split large resources into parts
numbered from zero, and the compressed stream must be concatenated before
inflation; reading only part zero cannot decode such resources at all.

## What Changes

- Detect the storage layout with one SELECT-only catalog query per provider
  (`COL_LENGTH`/`OBJECT_ID` on MSSQL, `pg_attribute`/`pg_class` on
  PostgreSQL) that cannot fail, and expose the result as `StorageLayout`.
- Provide legacy query variants without `PartNo` that return the same
  `(name, part, data)` row shape as the modern variants, so adapters share
  one reader.
- Assemble multi-part resources by concatenating `BinaryData` in ascending
  `PartNo` before decoding; a gap in the part sequence is a data error.
- Skip extension acquisition when `ConfigCAS` or `_ExtensionsRestruct` are
  absent, and report a typed error when `SchemaStorage` is absent.
- Progress totals count distinct resources and the bytes of every part.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: layout detection, multi-part assembly, legacy query
  variants.

## Impact

- Public API: `StorageLayout`, layout query constants, selector methods on
  `PostgresMetadataQueries`/`MsSqlMetadataQueries`; `all()` arrays grow.
- CLI adapters gain a layout probe after the transaction starts.
- No new dependencies; decoders are unchanged.
