## Context

`src/metadata/queries.rs` hard-codes `PartNo = 0` in the DBNames, Config,
Config-totals, and ConfigCAS statements and reads `ConfigCAS` and
`_ExtensionsRestruct` unconditionally. A live 8.2 base on SQL Server 2019 was
inspected through `sys.columns`: the file tables have exactly `FileName,
Creation, Modified, Attributes, DataSize, BinaryData` (`image`), the extension
tables do not exist, and `SchemaStorage`/`_YearOffset` are compatible.

Research on Tool1CD, DaJet, and py1cv8 established that parts are numbered
contiguously from zero per `FileName`, that the deflate stream spans the
concatenation of all parts, and that whether `Params` carries `PartNo` must be
checked per table.

## Decisions

### Catalog probe instead of try-and-fail

PostgreSQL acquisition runs in one read-only transaction without savepoints,
and the CLI's error conversion drops SQLSTATE, so a failing probe would abort
the transaction. Both providers therefore run one catalog query that always
succeeds and returns six integer flags: `Params.PartNo`, `Config.PartNo`,
`ConfigCAS` exists, `ConfigCAS.PartNo`, `_ExtensionsRestruct` exists,
`SchemaStorage` exists. PostgreSQL uses `pg_attribute`/`pg_class` joins rather
than `to_regclass`, which is absent on the PostgreSQL 8.4–9.2 servers that
platform 8.2 supported.

### One row shape for both layouts

Legacy variants select a constant `0` part number, so adapters always read
`(name, part, data)` for Config/ConfigCAS and `(part, data)` for DBNames.
Modern variants order by `(FileName, PartNo)`; the shared `assemble_parts`
reader groups consecutive rows of one name, verifies the sequence
`0, 1, 2, …`, and emits the concatenated resource. Progress totals use
`COUNT(DISTINCT FileName)` and the sum over all parts so the denominator
matches the number of assembled resources.

### PostgreSQL file names

`filename` may be blank-padded on old servers, so PostgreSQL variants compare
and return `rtrim(filename::text)`; the function is harmless on modern bases.

### Missing tables

`ConfigCAS` and `_ExtensionsRestruct` absent means "no extensions": the
adapters return empty lists without issuing the queries. `SchemaStorage`
absent makes the base unqueryable, so `StorageLayout::require_schema_storage`
returns a typed `MetadataError` before any statement is run.

## Risks / Trade-offs

- The exact platform build that added `PartNo` on SQL backends is not
  documented; detection by catalog avoids depending on it.
- `.1c` predefined-value resources have not been observed on 8.2; the
  predicate keeps accepting them, so a base without them simply yields none.
