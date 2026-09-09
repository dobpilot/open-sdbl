use crate::metadata::ObjectId;
use crate::query::core::codegen::context::{JoinKey, JoinPlan};
use crate::query::core::dialect::{LabelLimit, OutputLabelAllocator, SqlDialect, truncate_label};
use crate::query::core::names::names_equal;

#[test]
fn metadata_names_use_one_allocation_free_case_fold() {
    assert!(names_equal("Справочник", "справочник"));
    assert!(names_equal("ASCII_Name", "ascii_name"));
    assert!(names_equal("İ", "i\u{307}"));
    assert!(!names_equal("Код", "Коды"));
}

#[test]
fn join_reuse_requires_the_complete_reference_identity() {
    let target = ObjectId::from_bytes([1; 16]);
    let join = JoinPlan {
        source_alias: "source".to_owned(),
        source_field: "Reference".to_owned(),
        source_column: "_Fld1_RRRef".to_owned(),
        source_type_column: Some("_Fld1_RTRef".to_owned()),
        database_type: Some(57),
        target_object: target,
        target_relation: "_Reference57".to_owned(),
        target_id_column: "_IDRRef".to_owned(),
        alias: "__ref1".to_owned(),
    };
    let matching = JoinKey {
        source_alias: "SOURCE",
        source_field: "reference",
        target_object: target,
        database_type: Some(57),
    };
    assert!(join.matches(matching));
    assert!(!join.matches(JoinKey {
        database_type: None,
        ..matching
    }));
    assert!(!join.matches(JoinKey {
        target_object: ObjectId::from_bytes([2; 16]),
        ..matching
    }));
    assert!(!join.matches(JoinKey {
        source_alias: "other",
        ..matching
    }));
}

#[test]
fn quotes_identifiers_with_the_canonical_dialect_delimiters() {
    assert_eq!(SqlDialect::Postgres.quote_identifier("a\"b"), "\"a\"\"b\"");
    assert_eq!(
        SqlDialect::mssql(0, crate::query::mssql::MsSqlDialectLevel::Sql2012)
            .quote_identifier("a]b"),
        "[a]]b]"
    );
}

#[test]
fn bounds_mssql_labels_by_utf16_code_units() {
    let requested = "😀".repeat(100);
    let truncated = truncate_label(&requested, LabelLimit::Utf16Units(128));
    assert_eq!(truncated.encode_utf16().count(), 128);
    assert_eq!(truncated.chars().count(), 64);

    let mut labels = OutputLabelAllocator::new(SqlDialect::mssql(
        0,
        crate::query::mssql::MsSqlDialectLevel::Sql2012,
    ));
    let first = labels.allocate(&requested);
    let second = labels.allocate(&requested);
    assert_ne!(first, second);
    assert!(first.encode_utf16().count() <= 128);
    assert!(second.encode_utf16().count() <= 128);
}

#[test]
fn classifies_live_catalog_types_from_both_providers() {
    use crate::query::core::resolve::ColumnKind;

    assert_eq!(
        ColumnKind::from_catalog_type("numeric(10,2)"),
        ColumnKind::Number {
            precision: Some(10),
            scale: Some(2),
        }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("mvarchar(9)"),
        ColumnKind::String { length: Some(9) }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("character varying(150)"),
        ColumnKind::String { length: Some(150) }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("nvarchar(max)"),
        ColumnKind::String { length: None }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("timestamp without time zone"),
        ColumnKind::DateTime
    );
    assert_eq!(
        ColumnKind::from_catalog_type("datetime2"),
        ColumnKind::DateTime
    );
    assert_eq!(
        ColumnKind::from_catalog_type("timestamp"),
        ColumnKind::Binary { length: Some(8) }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("binary(16)"),
        ColumnKind::Binary { length: Some(16) }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("bytea"),
        ColumnKind::Binary { length: None }
    );
    assert_eq!(
        ColumnKind::from_catalog_type("int"),
        ColumnKind::Number {
            precision: Some(10),
            scale: Some(0),
        }
    );
    assert_eq!(ColumnKind::from_catalog_type("bit"), ColumnKind::Boolean);
    assert_eq!(
        ColumnKind::from_catalog_type("boolean"),
        ColumnKind::Boolean
    );
    assert_eq!(
        ColumnKind::from_catalog_type("uniqueidentifier"),
        ColumnKind::Uuid
    );
    assert_eq!(
        ColumnKind::from_catalog_type("xml"),
        ColumnKind::Unknown {
            data_type: "xml".to_owned()
        }
    );
    assert!(ColumnKind::Null.is_compatible_with(&ColumnKind::DateTime));
    assert!(
        ColumnKind::String { length: Some(1) }
            .is_compatible_with(&ColumnKind::String { length: None })
    );
    assert!(!ColumnKind::Boolean.is_compatible_with(&ColumnKind::DateTime));
}
