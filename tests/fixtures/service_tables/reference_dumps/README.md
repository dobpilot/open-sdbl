# Whole reference metadata dumps

These are the complete resources captured during the phase-1 inventory from
the MSSQL and PostgreSQL reference bases documented in
`docs/service-tables-inventory.md`.

- `*/db_names.deflate` is the raw-DEFLATE `Params.DBNames` payload exactly as
  returned by the provider.
- `*/schema_storage.deflate` is the UTF-8-with-BOM
  `SchemaStorage.CurrentSchema` payload, recompressed as raw DEFLATE solely to
  keep the repository fixture small. The conformance test inflates it before
  parsing.

The decoded resources are intentionally not checked in. The fixture checksums
are:

```text
362a531a376e2d269049d6fad5493b9ae0936b3390dd949c659ad1fa8d018277  mssql/db_names.deflate
03aacb3aba16681d06e04e3d1d4ce5c94a8ffb545b55f7c2821d722140cf144d  mssql/schema_storage.deflate
85f97442196d6fed6f1d40962ea24836e5877c5ccfcd36b19c6e7dd8f6f67f44  postgres/db_names.deflate
5fff114f2977ff457448bb841a27ad0c2d3f5fe2826690e047867c9950330dbf  postgres/schema_storage.deflate
```
