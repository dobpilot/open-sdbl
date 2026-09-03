mod support;

use support::snapshot;

use open_sdbl::metadata::{AllowedLength, MetadataKind};

/// Resolves minimal projections captured after publishing
/// `Catalog.OpenSdblMetadataProbe` to the disposable 8.3.27/PostgreSQL
/// conformance information base.
#[test]
fn resolves_the_catalog_and_attribute_verified_in_the_test_infobase() {
    let snapshot = snapshot();
    let object = snapshot
        .objects
        .iter()
        .find(|object| object.name.as_deref() == Some("OpenSdblMetadataProbe"))
        .unwrap();
    assert_eq!(object.kind, Some(MetadataKind::Catalog));
    assert_eq!(object.physical_table.as_deref(), Some("_Reference53"));
    assert!(object.declared && object.live);
    assert_eq!(object.code_allowed_length, Some(AllowedLength::Variable));

    let field = &snapshot.fields[0];
    assert_eq!(field.name.as_deref(), Some("ProbeAttribute"));
    assert_eq!(field.physical_name, "_Fld54");
    assert_eq!(field.owner_tables, ["_Reference53"]);
    assert!(field.declared && field.live);

    assert_eq!(snapshot.indexes[0].logical_key, ["Code", "ID"]);
    assert_eq!(
        snapshot.indexes[0].live_name.as_deref(),
        Some("_reference53_2")
    );
    assert!(snapshot.indexes[0].unique_matches);
}
