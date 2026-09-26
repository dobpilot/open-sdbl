//! Regression checks for the PostgreSQL selected Config statements.

use open_sdbl::metadata::{Guid, StorageLayout};

use super::{
    ConfigBootstrapName, selected_postgres_bootstrap_sql, selected_postgres_remaining_sql,
};

#[test]
fn selected_config_predicates_use_validated_literals_for_mvarchar() {
    let descriptor = "12345678-1234-1234-1234-123456789abc"
        .parse::<Guid>()
        .unwrap();
    for layout in [StorageLayout::MODERN, StorageLayout::LEGACY] {
        let root = selected_postgres_bootstrap_sql(&layout, ConfigBootstrapName::Root);
        assert!(
            root.contains(" WHERE filename = 'root' ORDER BY "),
            "{root}"
        );
        assert!(!root.contains('$'), "{root}");
        let named =
            selected_postgres_bootstrap_sql(&layout, ConfigBootstrapName::Descriptor(&descriptor));
        assert!(
            named.contains(" WHERE filename = '12345678-1234-1234-1234-123456789abc' ORDER BY "),
            "{named}"
        );
        assert!(!named.contains('$'), "{named}");
        let remaining = selected_postgres_remaining_sql(&layout, &descriptor);
        assert!(remaining.contains(" WHERE filename <> 'root' AND filename <> '12345678-1234-1234-1234-123456789abc' ORDER BY "), "{remaining}");
        assert!(!remaining.contains('$'), "{remaining}");
    }
}
