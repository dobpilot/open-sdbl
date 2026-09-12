# Probe base with data separators

Resources of a minimal configuration loaded with `ibcmd` (platform
8.3.27.2342) into a PostgreSQL 18 infobase, captured on 2026-09-12:

- catalog `Товары` with attributes `Артикул` (string) and `Поставщик`
  (reference to `Товары`), table `_Reference53`;
- constant `ОсновнойТовар` (reference to `Товары`), table `_Const61`;
- common attributes in `Разделять` mode, both bound to the session
  parameters `ЗначениеРазделителя` (Number 7) and
  `ИспользованиеРазделителя` (Boolean): `РазделительНезависимо`
  (`Независимо`, `_Fld56`) and `РазделительСовместно` (`Независимо и
  совместно`, `_Fld57`).

Files:

- `db_names.deflate` — raw-DEFLATE `Params.DBNames` as returned by the
  provider;
- `schema_storage.txt` — `SchemaStorage.CurrentSchema` bytes (UTF-8 with
  BOM), uncompressed;
- `config/<filename>.deflate` — every part-zero `Config` row, including
  suffixed slots the decoder ignores;
- `live_columns.tsv` — `information_schema.columns` for the two object
  tables (`USER-DEFINED` stands for `mvarchar`);
- `live_indexes.tsv` — `pg_indexes` for the same tables, showing the
  separator columns leading every index and the primary key
  `(_fld56, _idrref)`.
