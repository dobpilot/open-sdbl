use super::{MetadataError, MetadataErrorKind};

/// Physical layout of the 1C service tables, detected from the database
/// catalog before any metadata statement runs.
///
/// Platform 8.2 and 8.3 builds before the 8.3.8 storage format keep
/// `Params`/`Config` without a `PartNo` column and have no extension tables;
/// modern bases split large resources into parts numbered from zero.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StorageLayout {
    /// `Params` carries a `PartNo` column.
    pub params_parts: bool,
    /// `Config` carries a `PartNo` column.
    pub config_parts: bool,
    /// `ConfigCAS` exists.
    pub extension_store: bool,
    /// `ConfigCAS` carries a `PartNo` column.
    pub extension_store_parts: bool,
    /// `_ExtensionsRestruct` exists.
    pub extension_restructure: bool,
    /// `SchemaStorage` exists.
    pub schema_storage: bool,
}

impl StorageLayout {
    /// Layout of a modern 8.3 base: every file table has parts and the
    /// extension tables exist.
    pub const MODERN: Self = Self {
        params_parts: true,
        config_parts: true,
        extension_store: true,
        extension_store_parts: true,
        extension_restructure: true,
        schema_storage: true,
    };

    /// Layout of a platform 8.2 base: single-row resources, no extensions.
    pub const LEGACY: Self = Self {
        params_parts: false,
        config_parts: false,
        extension_store: false,
        extension_store_parts: false,
        extension_restructure: false,
        schema_storage: true,
    };

    /// Builds a layout from the six integer flags returned by
    /// [`PostgresMetadataQueries::LAYOUT`] or [`MsSqlMetadataQueries::LAYOUT`],
    /// in that column order; any non-zero value means "present".
    #[must_use]
    pub const fn from_flags(flags: [i32; 6]) -> Self {
        Self {
            params_parts: flags[0] != 0,
            config_parts: flags[1] != 0,
            extension_store: flags[2] != 0,
            extension_store_parts: flags[3] != 0,
            extension_restructure: flags[4] != 0,
            schema_storage: flags[5] != 0,
        }
    }

    /// Fails when the base has no `SchemaStorage`, without which the physical
    /// schema cannot be resolved.
    ///
    /// # Errors
    ///
    /// Returns a [`MetadataErrorKind::Schema`] error naming the missing table.
    pub fn require_schema_storage(&self) -> Result<(), MetadataError> {
        if self.schema_storage {
            Ok(())
        } else {
            Err(MetadataError::new(
                MetadataErrorKind::Schema,
                "SchemaStorage is absent; the information base needs a platform restructure before it can be queried",
            ))
        }
    }

    /// Short human description used by progress output.
    #[must_use]
    pub const fn describe(&self) -> &'static str {
        if self.config_parts {
            "parts"
        } else {
            "legacy (no PartNo)"
        }
    }
}

/// Fixed SELECT-only queries needed to acquire 1C metadata from PostgreSQL.
///
/// The core library only provides query text. Opening a connection and
/// executing these statements are responsibilities of an application crate.
/// Statements that depend on the storage layout come in two variants that
/// return the same row shape; pick one through the selector methods.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostgresMetadataQueries;

impl PostgresMetadataQueries {
    /// Verifies the transaction mode selected by the database adapter.
    pub const VERIFY_TRANSACTION: &'static str =
        "SELECT current_setting('transaction_read_only'), current_setting('transaction_isolation')";

    /// Detects the storage layout as six integer flags: `params.partno`,
    /// `config.partno`, `configcas` exists, `configcas.partno`,
    /// `_extensionsrestruct` exists, `schemastorage` exists. Uses only
    /// `pg_class`/`pg_attribute`, which exist on every supported server.
    pub const LAYOUT: &'static str = "SELECT CASE WHEN EXISTS (SELECT 1 FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'params' AND a.attname = 'partno' AND NOT a.attisdropped) THEN 1 ELSE 0 END, CASE WHEN EXISTS (SELECT 1 FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'config' AND a.attname = 'partno' AND NOT a.attisdropped) THEN 1 ELSE 0 END, CASE WHEN EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'configcas' AND c.relkind IN ('r','p')) THEN 1 ELSE 0 END, CASE WHEN EXISTS (SELECT 1 FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'configcas' AND a.attname = 'partno' AND NOT a.attisdropped) THEN 1 ELSE 0 END, CASE WHEN EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = '_extensionsrestruct' AND c.relkind IN ('r','p')) THEN 1 ELSE 0 END, CASE WHEN EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = 'schemastorage' AND c.relkind IN ('r','p')) THEN 1 ELSE 0 END";

    /// Reads every part of the raw-DEFLATE DBNames resource as
    /// `(part, data)` rows in part order.
    pub const DB_NAMES: &'static str = "SELECT partno, binarydata FROM params WHERE rtrim(filename::text) = 'DBNames' ORDER BY partno";

    /// Reads the single-row DBNames resource of a base without `partno` as a
    /// `(part, data)` row with part zero.
    pub const DB_NAMES_LEGACY: &'static str =
        "SELECT 0::int, binarydata FROM params WHERE rtrim(filename::text) = 'DBNames'";

    /// Reads every part of bare-GUID descriptors and `.1c` predefined values
    /// as `(name, part, data)` rows ordered by name and part.
    pub const CONFIG: &'static str = "SELECT rtrim(filename::text), partno, binarydata FROM config WHERE rtrim(filename::text) ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(\\.1c)?$' ORDER BY filename, partno";

    /// Reads single-row descriptors of a base without `partno` as
    /// `(name, part, data)` rows with part zero.
    pub const CONFIG_LEGACY: &'static str = "SELECT rtrim(filename::text), 0::int, binarydata FROM config WHERE rtrim(filename::text) ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(\\.1c)?$' ORDER BY filename";

    /// Counts distinct resources and the compressed bytes of every part
    /// matched by [`Self::CONFIG`] or [`Self::CONFIG_LEGACY`].
    pub const CONFIG_TOTALS: &'static str = "SELECT count(DISTINCT filename), COALESCE(sum(octet_length(binarydata)), 0) FROM config WHERE rtrim(filename::text) ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(\\.1c)?$'";

    /// Reads every part of the opaque configuration-extension resources as
    /// `(name, part, data)` rows ordered by name and part.
    ///
    /// Their content-addressed graph is decoded by the application-side
    /// extension boundary, not by the database adapter.
    pub const EXTENSION_RESOURCES: &'static str =
        "SELECT rtrim(filename::text), partno, binarydata FROM configcas ORDER BY filename, partno";

    /// Reads single-row extension resources of a `configcas` without
    /// `partno` as `(name, part, data)` rows with part zero.
    pub const EXTENSION_RESOURCES_LEGACY: &'static str =
        "SELECT rtrim(filename::text), 0::int, binarydata FROM configcas ORDER BY filename";

    /// Reads extension restructure records mapping extension attributes to
    /// their physical `Fld` columns.
    pub const EXTENSION_RESTRUCTURE: &'static str =
        "SELECT _restructdata FROM _extensionsrestruct WHERE _restructdata IS NOT NULL";

    /// Reads the current authoritative physical schema.
    pub const SCHEMA: &'static str = "SELECT currentschema FROM schemastorage WHERE schemaid = 0";

    /// Reads public PostgreSQL tables, columns, and ordered index keys.
    pub const CATALOG: &'static str = "SELECT 'T', c.relname, '', '', '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relkind IN ('r','p') UNION ALL SELECT 'C', c.relname, a.attname, format_type(a.atttypid, a.atttypmod), '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace JOIN pg_attribute a ON a.attrelid = c.oid WHERE n.nspname = 'public' AND c.relkind IN ('r','p') AND a.attnum > 0 AND NOT a.attisdropped UNION ALL SELECT 'I', t.relname, i.relname, x.indisunique::text, COALESCE(string_agg(a.attname, ',' ORDER BY k.ordinality), '') FROM pg_class t JOIN pg_namespace n ON n.oid = t.relnamespace JOIN pg_index x ON x.indrelid = t.oid JOIN pg_class i ON i.oid = x.indexrelid LEFT JOIN LATERAL unnest(x.indkey) WITH ORDINALITY AS k(attnum, ordinality) ON true LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum WHERE n.nspname = 'public' GROUP BY t.relname, i.relname, x.indisunique ORDER BY 1, 2, 3";

    /// Selects the DBNames statement for a detected layout.
    #[must_use]
    pub const fn db_names(layout: &StorageLayout) -> &'static str {
        if layout.params_parts {
            Self::DB_NAMES
        } else {
            Self::DB_NAMES_LEGACY
        }
    }

    /// Selects the Config statement for a detected layout.
    #[must_use]
    pub const fn config(layout: &StorageLayout) -> &'static str {
        if layout.config_parts {
            Self::CONFIG
        } else {
            Self::CONFIG_LEGACY
        }
    }

    /// Selects the extension-resource statement for a detected layout, or
    /// `None` when the base has no `configcas`.
    #[must_use]
    pub const fn extension_resources(layout: &StorageLayout) -> Option<&'static str> {
        if !layout.extension_store {
            None
        } else if layout.extension_store_parts {
            Some(Self::EXTENSION_RESOURCES)
        } else {
            Some(Self::EXTENSION_RESOURCES_LEGACY)
        }
    }

    /// Selects the restructure statement, or `None` when the base has no
    /// `_extensionsrestruct`.
    #[must_use]
    pub const fn extension_restructure(layout: &StorageLayout) -> Option<&'static str> {
        if layout.extension_restructure {
            Some(Self::EXTENSION_RESTRUCTURE)
        } else {
            None
        }
    }

    /// Returns every acquisition statement, both layout variants included.
    #[must_use]
    pub const fn all() -> [&'static str; 12] {
        [
            Self::VERIFY_TRANSACTION,
            Self::LAYOUT,
            Self::DB_NAMES,
            Self::DB_NAMES_LEGACY,
            Self::CONFIG_TOTALS,
            Self::CONFIG,
            Self::CONFIG_LEGACY,
            Self::EXTENSION_RESOURCES,
            Self::EXTENSION_RESOURCES_LEGACY,
            Self::EXTENSION_RESTRUCTURE,
            Self::SCHEMA,
            Self::CATALOG,
        ]
    }
}

/// Fixed SELECT-only queries needed to acquire 1C metadata from Microsoft SQL
/// Server.
///
/// The core library only provides T-SQL text. Opening a TDS connection and
/// executing these statements are responsibilities of an application crate.
/// Statements that depend on the storage layout come in two variants that
/// return the same row shape; pick one through the selector methods.
#[derive(Debug, Clone, Copy, Default)]
pub struct MsSqlMetadataQueries;

impl MsSqlMetadataQueries {
    /// Reads the connected database name and status for adapter validation.
    pub const VERIFY_DATABASE: &'static str =
        "SELECT DB_NAME(), CONVERT(nvarchar(60), DATABASEPROPERTYEX(DB_NAME(), N'Status'))";

    /// Reads the year offset applied to physical 1C datetime values.
    pub const YEAR_OFFSET: &'static str =
        "SELECT TOP (1) CONVERT(int, [Offset]) FROM [dbo].[_YearOffset]";

    /// Detects the storage layout as six integer flags: `Params.PartNo`,
    /// `Config.PartNo`, `ConfigCAS` exists, `ConfigCAS.PartNo`,
    /// `_ExtensionsRestruct` exists, `SchemaStorage` exists.
    pub const LAYOUT: &'static str = "SELECT CASE WHEN COL_LENGTH(N'dbo.Params', N'PartNo') IS NULL THEN 0 ELSE 1 END, CASE WHEN COL_LENGTH(N'dbo.Config', N'PartNo') IS NULL THEN 0 ELSE 1 END, CASE WHEN OBJECT_ID(N'dbo.ConfigCAS', N'U') IS NULL THEN 0 ELSE 1 END, CASE WHEN COL_LENGTH(N'dbo.ConfigCAS', N'PartNo') IS NULL THEN 0 ELSE 1 END, CASE WHEN OBJECT_ID(N'dbo._ExtensionsRestruct', N'U') IS NULL THEN 0 ELSE 1 END, CASE WHEN OBJECT_ID(N'dbo.SchemaStorage', N'U') IS NULL THEN 0 ELSE 1 END";

    /// Reads every part of the raw-DEFLATE DBNames resource as
    /// `(part, data)` rows in part order.
    pub const DB_NAMES: &'static str = "SELECT [PartNo], [BinaryData] FROM [dbo].[Params] WHERE [FileName] = N'DBNames' ORDER BY [PartNo]";

    /// Reads the single-row DBNames resource of a base without `PartNo` as a
    /// `(part, data)` row with part zero.
    pub const DB_NAMES_LEGACY: &'static str =
        "SELECT CONVERT(int, 0), [BinaryData] FROM [dbo].[Params] WHERE [FileName] = N'DBNames'";

    /// Reads every part of canonical-GUID descriptors and `.1c` predefined
    /// values as `(name, part, data)` rows ordered by name and part.
    pub const CONFIG: &'static str = "SELECT CONVERT(nvarchar(128), [FileName]), [PartNo], [BinaryData] FROM [dbo].[Config] WHERE ((LEN([FileName]) = 36 AND TRY_CONVERT(uniqueidentifier, [FileName]) IS NOT NULL) OR (LEN([FileName]) = 39 AND RIGHT([FileName], 3) = N'.1c' AND TRY_CONVERT(uniqueidentifier, LEFT([FileName], 36)) IS NOT NULL)) ORDER BY [FileName], [PartNo]";

    /// Reads single-row descriptors of a base without `PartNo` as
    /// `(name, part, data)` rows with part zero.
    pub const CONFIG_LEGACY: &'static str = "SELECT CONVERT(nvarchar(128), [FileName]), CONVERT(int, 0), [BinaryData] FROM [dbo].[Config] WHERE ((LEN([FileName]) = 36 AND TRY_CONVERT(uniqueidentifier, [FileName]) IS NOT NULL) OR (LEN([FileName]) = 39 AND RIGHT([FileName], 3) = N'.1c' AND TRY_CONVERT(uniqueidentifier, LEFT([FileName], 36)) IS NOT NULL)) ORDER BY [FileName]";

    /// Counts distinct resources and the compressed bytes of every part
    /// matched by [`Self::CONFIG`] or [`Self::CONFIG_LEGACY`].
    pub const CONFIG_TOTALS: &'static str = "SELECT COUNT_BIG(DISTINCT [FileName]), COALESCE(SUM(CONVERT(bigint, DATALENGTH([BinaryData]))), CONVERT(bigint, 0)) FROM [dbo].[Config] WHERE ((LEN([FileName]) = 36 AND TRY_CONVERT(uniqueidentifier, [FileName]) IS NOT NULL) OR (LEN([FileName]) = 39 AND RIGHT([FileName], 3) = N'.1c' AND TRY_CONVERT(uniqueidentifier, LEFT([FileName], 36)) IS NOT NULL))";

    /// Reads every part of the opaque configuration-extension resources as
    /// `(name, part, data)` rows ordered by name and part.
    ///
    /// Their content-addressed graph is decoded by the application-side
    /// extension boundary, not by the database adapter.
    pub const EXTENSION_RESOURCES: &'static str = "SELECT CONVERT(nvarchar(128), [FileName]), [PartNo], [BinaryData] FROM [dbo].[ConfigCAS] ORDER BY [FileName], [PartNo]";

    /// Reads single-row extension resources of a `ConfigCAS` without
    /// `PartNo` as `(name, part, data)` rows with part zero.
    pub const EXTENSION_RESOURCES_LEGACY: &'static str = "SELECT CONVERT(nvarchar(128), [FileName]), CONVERT(int, 0), [BinaryData] FROM [dbo].[ConfigCAS] ORDER BY [FileName]";

    /// Reads extension restructure records mapping extension attributes to
    /// their physical `Fld` columns.
    pub const EXTENSION_RESTRUCTURE: &'static str =
        "SELECT [_RestructData] FROM [dbo].[_ExtensionsRestruct] WHERE [_RestructData] IS NOT NULL";

    /// Reads the current authoritative physical schema.
    pub const SCHEMA: &'static str =
        "SELECT [CurrentSchema] FROM [dbo].[SchemaStorage] WHERE [SchemaID] = 0";

    /// Reads `dbo` SQL Server tables, columns, and ordered index keys.
    pub const CATALOG: &'static str = "SELECT N'T', t.[name], N'', N'', N'' FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] WHERE s.[name] = N'dbo' UNION ALL SELECT N'C', t.[name], c.[name], CASE WHEN ty.[name] IN (N'nvarchar', N'nchar') THEN ty.[name] + N'(' + CASE WHEN c.[max_length] = -1 THEN N'max' ELSE CONVERT(nvarchar(10), c.[max_length] / 2) END + N')' WHEN ty.[name] IN (N'varchar', N'char', N'varbinary', N'binary') THEN ty.[name] + N'(' + CASE WHEN c.[max_length] = -1 THEN N'max' ELSE CONVERT(nvarchar(10), c.[max_length]) END + N')' WHEN ty.[name] IN (N'decimal', N'numeric') THEN ty.[name] + N'(' + CONVERT(nvarchar(10), c.[precision]) + N',' + CONVERT(nvarchar(10), c.[scale]) + N')' ELSE ty.[name] END, N'' FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] INNER JOIN sys.columns AS c ON c.[object_id] = t.[object_id] INNER JOIN sys.types AS ty ON ty.[user_type_id] = c.[user_type_id] WHERE s.[name] = N'dbo' UNION ALL SELECT N'I', t.[name], i.[name], CASE WHEN i.[is_unique] = 1 THEN N'true' ELSE N'false' END, COALESCE(STUFF((SELECT N',' + c2.[name] FROM sys.index_columns AS ic2 INNER JOIN sys.columns AS c2 ON c2.[object_id] = ic2.[object_id] AND c2.[column_id] = ic2.[column_id] WHERE ic2.[object_id] = i.[object_id] AND ic2.[index_id] = i.[index_id] AND ic2.[key_ordinal] > 0 AND ic2.[is_included_column] = 0 ORDER BY ic2.[key_ordinal] FOR XML PATH(N''), TYPE).value(N'.', N'nvarchar(max)'), 1, 1, N''), N'') FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] INNER JOIN sys.indexes AS i ON i.[object_id] = t.[object_id] WHERE s.[name] = N'dbo' AND i.[index_id] > 0 AND i.[is_hypothetical] = 0 ORDER BY 1, 2, 3";

    /// Selects the DBNames statement for a detected layout.
    #[must_use]
    pub const fn db_names(layout: &StorageLayout) -> &'static str {
        if layout.params_parts {
            Self::DB_NAMES
        } else {
            Self::DB_NAMES_LEGACY
        }
    }

    /// Selects the Config statement for a detected layout.
    #[must_use]
    pub const fn config(layout: &StorageLayout) -> &'static str {
        if layout.config_parts {
            Self::CONFIG
        } else {
            Self::CONFIG_LEGACY
        }
    }

    /// Selects the extension-resource statement for a detected layout, or
    /// `None` when the base has no `ConfigCAS`.
    #[must_use]
    pub const fn extension_resources(layout: &StorageLayout) -> Option<&'static str> {
        if !layout.extension_store {
            None
        } else if layout.extension_store_parts {
            Some(Self::EXTENSION_RESOURCES)
        } else {
            Some(Self::EXTENSION_RESOURCES_LEGACY)
        }
    }

    /// Selects the restructure statement, or `None` when the base has no
    /// `_ExtensionsRestruct`.
    #[must_use]
    pub const fn extension_restructure(layout: &StorageLayout) -> Option<&'static str> {
        if layout.extension_restructure {
            Some(Self::EXTENSION_RESTRUCTURE)
        } else {
            None
        }
    }

    /// Returns every acquisition statement, both layout variants included.
    #[must_use]
    pub const fn all() -> [&'static str; 13] {
        [
            Self::VERIFY_DATABASE,
            Self::YEAR_OFFSET,
            Self::LAYOUT,
            Self::DB_NAMES,
            Self::DB_NAMES_LEGACY,
            Self::CONFIG_TOTALS,
            Self::CONFIG,
            Self::CONFIG_LEGACY,
            Self::EXTENSION_RESOURCES,
            Self::EXTENSION_RESOURCES_LEGACY,
            Self::EXTENSION_RESTRUCTURE,
            Self::SCHEMA,
            Self::CATALOG,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{MsSqlMetadataQueries, PostgresMetadataQueries, StorageLayout};

    #[test]
    fn every_acquisition_query_is_select_only() {
        for query in PostgresMetadataQueries::all()
            .into_iter()
            .chain(MsSqlMetadataQueries::all())
        {
            let normalized = query.trim().to_ascii_uppercase();
            assert!(normalized.starts_with("SELECT "));
            for mutating in ["INSERT ", "UPDATE ", "DELETE ", "ALTER ", "DROP "] {
                assert!(!normalized.contains(mutating));
            }
        }
    }

    #[test]
    fn extension_queries_use_the_captured_config_cas_shape() {
        for query in [
            PostgresMetadataQueries::EXTENSION_RESOURCES,
            MsSqlMetadataQueries::EXTENSION_RESOURCES,
        ] {
            let normalized = query.to_ascii_uppercase();
            for token in ["CONFIGCAS", "FILENAME", "BINARYDATA", "PARTNO"] {
                assert!(normalized.contains(token), "{token}: {query}");
            }
            assert!(normalized.contains("ORDER BY"));
        }
    }

    #[test]
    fn legacy_variants_never_mention_part_numbers() {
        for query in [
            PostgresMetadataQueries::DB_NAMES_LEGACY,
            PostgresMetadataQueries::CONFIG_LEGACY,
            PostgresMetadataQueries::CONFIG_TOTALS,
            PostgresMetadataQueries::EXTENSION_RESOURCES_LEGACY,
            MsSqlMetadataQueries::DB_NAMES_LEGACY,
            MsSqlMetadataQueries::CONFIG_LEGACY,
            MsSqlMetadataQueries::CONFIG_TOTALS,
            MsSqlMetadataQueries::EXTENSION_RESOURCES_LEGACY,
        ] {
            assert!(!query.to_ascii_uppercase().contains("PARTNO"), "{query}");
        }
        for query in [
            PostgresMetadataQueries::DB_NAMES,
            PostgresMetadataQueries::CONFIG,
            MsSqlMetadataQueries::DB_NAMES,
            MsSqlMetadataQueries::CONFIG,
        ] {
            assert!(query.to_ascii_uppercase().contains("ORDER BY"), "{query}");
        }
    }

    #[test]
    fn layout_probes_use_portable_catalog_functions() {
        let mssql = MsSqlMetadataQueries::LAYOUT;
        assert_eq!(mssql.matches("COL_LENGTH(").count(), 3);
        assert_eq!(mssql.matches("OBJECT_ID(").count(), 3);
        let postgres = PostgresMetadataQueries::LAYOUT;
        assert_eq!(postgres.matches("EXISTS (").count(), 6);
        assert!(postgres.contains("pg_attribute") && postgres.contains("pg_class"));
        assert!(!postgres.contains("to_regclass"));
        assert!(!postgres.contains("information_schema"));
    }

    #[test]
    fn selectors_follow_the_detected_layout() {
        let legacy = StorageLayout::from_flags([0, 0, 0, 0, 0, 1]);
        assert_eq!(legacy, StorageLayout::LEGACY);
        assert_eq!(
            MsSqlMetadataQueries::db_names(&legacy),
            MsSqlMetadataQueries::DB_NAMES_LEGACY
        );
        assert_eq!(
            PostgresMetadataQueries::config(&legacy),
            PostgresMetadataQueries::CONFIG_LEGACY
        );
        assert!(MsSqlMetadataQueries::extension_resources(&legacy).is_none());
        assert!(PostgresMetadataQueries::extension_restructure(&legacy).is_none());
        assert!(legacy.require_schema_storage().is_ok());
        assert_eq!(legacy.describe(), "legacy (no PartNo)");

        let modern = StorageLayout::from_flags([1, 1, 1, 1, 1, 1]);
        assert_eq!(modern, StorageLayout::MODERN);
        assert_eq!(
            MsSqlMetadataQueries::config(&modern),
            MsSqlMetadataQueries::CONFIG
        );
        assert_eq!(
            PostgresMetadataQueries::extension_resources(&modern),
            Some(PostgresMetadataQueries::EXTENSION_RESOURCES)
        );
        assert_eq!(modern.describe(), "parts");

        let store_without_parts = StorageLayout::from_flags([1, 1, 1, 0, 1, 1]);
        assert_eq!(
            MsSqlMetadataQueries::extension_resources(&store_without_parts),
            Some(MsSqlMetadataQueries::EXTENSION_RESOURCES_LEGACY)
        );

        let no_schema = StorageLayout::from_flags([0, 0, 0, 0, 0, 0]);
        let error = no_schema.require_schema_storage().unwrap_err();
        assert!(error.to_string().contains("SchemaStorage"));
    }
}
