use open_sdbl::metadata::{
    MsSqlMetadataQueries, PostgresMetadataQueries, is_config_metadata_resource,
};

fn filter(query: &str) -> Option<String> {
    let normalized = query.to_ascii_uppercase();
    normalized.split_once(" WHERE ").map(|(_, predicate)| {
        predicate
            .split(" ORDER BY ")
            .next()
            .unwrap()
            .trim()
            .to_owned()
    })
}

#[test]
fn each_provider_fingerprint_covers_the_filtered_metadata_rows_only() {
    for (fingerprint, metadata, whole) in [
        (
            PostgresMetadataQueries::CONFIG_FINGERPRINT,
            PostgresMetadataQueries::CONFIG,
            PostgresMetadataQueries::CONFIG_ALL,
        ),
        (
            MsSqlMetadataQueries::CONFIG_FINGERPRINT,
            MsSqlMetadataQueries::CONFIG,
            MsSqlMetadataQueries::CONFIG_ALL,
        ),
    ] {
        assert!(filter(fingerprint).is_some());
        assert_eq!(filter(fingerprint), filter(metadata));
        assert!(filter(whole).is_none());
        let normalized = fingerprint.to_ascii_uppercase();
        for table in ["CONFIGCAS", "_EXTENSIONSINFO", "_EXTENSIONSRESTRUCT"] {
            assert!(!normalized.contains(table));
        }
    }
}

#[test]
fn code_and_service_resources_outside_the_metadata_filter_are_excluded() {
    let id = "b8bac76b-c91b-4d78-8a70-ffa39f8de694";
    for suffix in ["", ".1c", ".9", ".7"] {
        assert!(is_config_metadata_resource(&format!("{id}{suffix}")));
    }
    for suffix in [".0", ".1", ".2", ".bin"] {
        assert!(!is_config_metadata_resource(&format!("{id}{suffix}")));
    }
    for name in ["DBNames", "root", "commonpicture.bin"] {
        assert!(!is_config_metadata_resource(name));
    }
}
