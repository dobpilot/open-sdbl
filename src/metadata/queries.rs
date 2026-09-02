/// Fixed SELECT-only queries needed to acquire 1C metadata from PostgreSQL.
///
/// The core library only provides query text. Opening a connection and
/// executing these statements are responsibilities of an application crate.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostgresMetadataQueries;

impl PostgresMetadataQueries {
    /// Verifies the transaction mode selected by the database adapter.
    pub const VERIFY_TRANSACTION: &'static str =
        "SELECT current_setting('transaction_read_only'), current_setting('transaction_isolation')";

    /// Reads the authoritative raw-DEFLATE DBNames resource.
    pub const DB_NAMES: &'static str =
        "SELECT binarydata FROM params WHERE filename = 'DBNames' AND partno = 0";

    /// Reads part-zero bare-GUID descriptors and `.1c` predefined values.
    pub const CONFIG: &'static str = "SELECT filename::text, binarydata FROM config WHERE partno = 0 AND filename::text ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(\\.1c)?$'";

    /// Counts the resources and compressed bytes returned by [`Self::CONFIG`].
    pub const CONFIG_TOTALS: &'static str = "SELECT count(*), COALESCE(sum(octet_length(binarydata)), 0) FROM config WHERE partno = 0 AND filename::text ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(\\.1c)?$'";

    /// Reads the current authoritative physical schema.
    pub const SCHEMA: &'static str = "SELECT currentschema FROM schemastorage WHERE schemaid = 0";

    /// Reads public PostgreSQL tables, columns, and ordered index keys.
    pub const CATALOG: &'static str = "SELECT 'T', c.relname, '', '', '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relkind IN ('r','p') UNION ALL SELECT 'C', c.relname, a.attname, format_type(a.atttypid, a.atttypmod), '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace JOIN pg_attribute a ON a.attrelid = c.oid WHERE n.nspname = 'public' AND c.relkind IN ('r','p') AND a.attnum > 0 AND NOT a.attisdropped UNION ALL SELECT 'I', t.relname, i.relname, x.indisunique::text, COALESCE(string_agg(a.attname, ',' ORDER BY k.ordinality), '') FROM pg_class t JOIN pg_namespace n ON n.oid = t.relnamespace JOIN pg_index x ON x.indrelid = t.oid JOIN pg_class i ON i.oid = x.indexrelid LEFT JOIN LATERAL unnest(x.indkey) WITH ORDINALITY AS k(attnum, ordinality) ON true LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum WHERE n.nspname = 'public' GROUP BY t.relname, i.relname, x.indisunique ORDER BY 1, 2, 3";

    /// Returns all acquisition statements in execution order.
    #[must_use]
    pub const fn all() -> [&'static str; 6] {
        [
            Self::VERIFY_TRANSACTION,
            Self::DB_NAMES,
            Self::CONFIG_TOTALS,
            Self::CONFIG,
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
#[derive(Debug, Clone, Copy, Default)]
pub struct MsSqlMetadataQueries;

impl MsSqlMetadataQueries {
    /// Reads the connected database name and status for adapter validation.
    pub const VERIFY_DATABASE: &'static str =
        "SELECT DB_NAME(), CONVERT(nvarchar(60), DATABASEPROPERTYEX(DB_NAME(), N'Status'))";

    /// Reads the year offset applied to physical 1C datetime values.
    pub const YEAR_OFFSET: &'static str =
        "SELECT TOP (1) CONVERT(int, [Offset]) FROM [dbo].[_YearOffset]";

    /// Reads the authoritative raw-DEFLATE DBNames resource.
    pub const DB_NAMES: &'static str =
        "SELECT [BinaryData] FROM [dbo].[Params] WHERE [FileName] = N'DBNames' AND [PartNo] = 0";

    /// Reads part-zero canonical-GUID descriptors and `.1c` predefined values.
    pub const CONFIG: &'static str = "SELECT CONVERT(nvarchar(128), [FileName]), [BinaryData] FROM [dbo].[Config] WHERE [PartNo] = 0 AND ((LEN([FileName]) = 36 AND TRY_CONVERT(uniqueidentifier, [FileName]) IS NOT NULL) OR (LEN([FileName]) = 39 AND RIGHT([FileName], 3) = N'.1c' AND TRY_CONVERT(uniqueidentifier, LEFT([FileName], 36)) IS NOT NULL)) ORDER BY [FileName]";

    /// Counts the resources and compressed bytes returned by [`Self::CONFIG`].
    pub const CONFIG_TOTALS: &'static str = "SELECT COUNT_BIG(*), COALESCE(SUM(CONVERT(bigint, DATALENGTH([BinaryData]))), CONVERT(bigint, 0)) FROM [dbo].[Config] WHERE [PartNo] = 0 AND ((LEN([FileName]) = 36 AND TRY_CONVERT(uniqueidentifier, [FileName]) IS NOT NULL) OR (LEN([FileName]) = 39 AND RIGHT([FileName], 3) = N'.1c' AND TRY_CONVERT(uniqueidentifier, LEFT([FileName], 36)) IS NOT NULL))";

    /// Reads the current authoritative physical schema.
    pub const SCHEMA: &'static str =
        "SELECT [CurrentSchema] FROM [dbo].[SchemaStorage] WHERE [SchemaID] = 0";

    /// Reads `dbo` SQL Server tables, columns, and ordered index keys.
    pub const CATALOG: &'static str = "SELECT N'T', t.[name], N'', N'', N'' FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] WHERE s.[name] = N'dbo' UNION ALL SELECT N'C', t.[name], c.[name], CASE WHEN ty.[name] IN (N'nvarchar', N'nchar') THEN ty.[name] + N'(' + CASE WHEN c.[max_length] = -1 THEN N'max' ELSE CONVERT(nvarchar(10), c.[max_length] / 2) END + N')' WHEN ty.[name] IN (N'varchar', N'char', N'varbinary', N'binary') THEN ty.[name] + N'(' + CASE WHEN c.[max_length] = -1 THEN N'max' ELSE CONVERT(nvarchar(10), c.[max_length]) END + N')' WHEN ty.[name] IN (N'decimal', N'numeric') THEN ty.[name] + N'(' + CONVERT(nvarchar(10), c.[precision]) + N',' + CONVERT(nvarchar(10), c.[scale]) + N')' ELSE ty.[name] END, N'' FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] INNER JOIN sys.columns AS c ON c.[object_id] = t.[object_id] INNER JOIN sys.types AS ty ON ty.[user_type_id] = c.[user_type_id] WHERE s.[name] = N'dbo' UNION ALL SELECT N'I', t.[name], i.[name], CASE WHEN i.[is_unique] = 1 THEN N'true' ELSE N'false' END, COALESCE(STUFF((SELECT N',' + c2.[name] FROM sys.index_columns AS ic2 INNER JOIN sys.columns AS c2 ON c2.[object_id] = ic2.[object_id] AND c2.[column_id] = ic2.[column_id] WHERE ic2.[object_id] = i.[object_id] AND ic2.[index_id] = i.[index_id] AND ic2.[key_ordinal] > 0 AND ic2.[is_included_column] = 0 ORDER BY ic2.[key_ordinal] FOR XML PATH(N''), TYPE).value(N'.', N'nvarchar(max)'), 1, 1, N''), N'') FROM sys.tables AS t INNER JOIN sys.schemas AS s ON s.[schema_id] = t.[schema_id] INNER JOIN sys.indexes AS i ON i.[object_id] = t.[object_id] WHERE s.[name] = N'dbo' AND i.[index_id] > 0 AND i.[is_hypothetical] = 0 ORDER BY 1, 2, 3";

    /// Returns all acquisition statements in execution order.
    #[must_use]
    pub const fn all() -> [&'static str; 7] {
        [
            Self::VERIFY_DATABASE,
            Self::YEAR_OFFSET,
            Self::DB_NAMES,
            Self::CONFIG_TOTALS,
            Self::CONFIG,
            Self::SCHEMA,
            Self::CATALOG,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{MsSqlMetadataQueries, PostgresMetadataQueries};

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
}
