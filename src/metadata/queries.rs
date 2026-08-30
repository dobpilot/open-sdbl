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

    /// Reads every part-zero resource whose file name is a bare GUID.
    pub const CONFIG: &'static str = "SELECT filename::text, binarydata FROM config WHERE partno = 0 AND filename::text ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$' ORDER BY filename";

    /// Reads the current authoritative physical schema.
    pub const SCHEMA: &'static str = "SELECT currentschema FROM schemastorage WHERE schemaid = 0";

    /// Reads public PostgreSQL tables, columns, and ordered index keys.
    pub const CATALOG: &'static str = "SELECT 'T', c.relname, '', '', '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relkind IN ('r','p') UNION ALL SELECT 'C', c.relname, a.attname, format_type(a.atttypid, a.atttypmod), '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace JOIN pg_attribute a ON a.attrelid = c.oid WHERE n.nspname = 'public' AND c.relkind IN ('r','p') AND a.attnum > 0 AND NOT a.attisdropped UNION ALL SELECT 'I', t.relname, i.relname, x.indisunique::text, COALESCE(string_agg(a.attname, ',' ORDER BY k.ordinality), '') FROM pg_class t JOIN pg_namespace n ON n.oid = t.relnamespace JOIN pg_index x ON x.indrelid = t.oid JOIN pg_class i ON i.oid = x.indexrelid LEFT JOIN LATERAL unnest(x.indkey) WITH ORDINALITY AS k(attnum, ordinality) ON true LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum WHERE n.nspname = 'public' GROUP BY t.relname, i.relname, x.indisunique ORDER BY 1, 2, 3";

    /// Returns all acquisition statements in execution order.
    #[must_use]
    pub const fn all() -> [&'static str; 5] {
        [
            Self::VERIFY_TRANSACTION,
            Self::DB_NAMES,
            Self::CONFIG,
            Self::SCHEMA,
            Self::CATALOG,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresMetadataQueries;

    #[test]
    fn every_acquisition_query_is_select_only() {
        for query in PostgresMetadataQueries::all() {
            let normalized = query.trim().to_ascii_uppercase();
            assert!(normalized.starts_with("SELECT "));
            for mutating in ["INSERT ", "UPDATE ", "DELETE ", "ALTER ", "DROP "] {
                assert!(!normalized.contains(mutating));
            }
        }
    }
}
