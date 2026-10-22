use open_sdbl::metadata::MetadataKind;
use open_sdbl::query::{QueryConstantReferences as Constants, query_metadata_references};

#[test]
fn discovers_direct_objects_from_existing_nested_expression_ast_only() {
    let source = r#"SELECT CAST(A.Ref AS Catalog.Target).Code,
        VALUE(Enumeration.State.Ready), TYPE(Catalog.Target),
        CASE WHEN A.Ref REFS Catalog.Other THEN 1 ELSE 0 END
        FROM (SELECT Ref FROM Catalog.Source) AS A
        WHERE A.Ref IN (SELECT Ref FROM catalog.target)
        AND "Catalog.Fake" = "Catalog.Fake""#;
    let result = query_metadata_references(source).unwrap();
    assert_eq!(
        result
            .objects
            .iter()
            .map(|o| (o.kind, o.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (MetadataKind::Catalog, "Source"),
            (MetadataKind::Catalog, "Target"),
            (MetadataKind::Enumeration, "State"),
            (MetadataKind::Catalog, "Other")
        ]
    );
    assert_eq!(result.constants, Constants::None);
}

#[test]
fn constants_distinguish_qualified_fields_labels_stars_and_ambiguity() {
    for source in [
        "SELECT C.Foo AS Alias FROM Constants AS C ORDER BY Alias",
        "ВЫБРАТЬ К.Режим КАК Имя ИЗ Константы К УПОРЯДОЧИТЬ ПО Имя",
    ] {
        let result = query_metadata_references(source).unwrap();
        assert!(result.objects.is_empty());
        assert!(
            matches!(result.constants, Constants::Named(names) if names.len()==1 && names[0]!="Alias" && names[0]!="Имя")
        );
    }
    let result = query_metadata_references("SELECT Foo, foo FROM Constants").unwrap();
    assert_eq!(result.constants, Constants::Named(vec!["Foo".into()]));
    for source in [
        "SELECT * FROM Constants",
        "SELECT C.* FROM Constants C",
        "SELECT Foo FROM Constants C JOIN Catalog.X X ON 1=1",
    ] {
        assert_eq!(
            query_metadata_references(source).unwrap().constants,
            Constants::All
        );
    }
    let result =
        query_metadata_references("SELECT X.Code FROM Constants C JOIN Catalog.X X ON 1=1")
            .unwrap();
    assert_eq!(result.constants, Constants::None);
}

#[test]
fn temporary_tables_literals_and_parameters_do_not_become_metadata() {
    let result =
        query_metadata_references("SELECT 1 AS X INTO T; SELECT T.X FROM T; УНИЧТОЖИТЬ T").unwrap();
    assert!(result.objects.is_empty());
    let result = query_metadata_references("SELECT P.X FROM &Rows AS P").unwrap();
    assert!(result.objects.is_empty());
}

#[test]
fn malformed_and_overdeep_queries_keep_positional_diagnostics() {
    let error = query_metadata_references("SELECT\nFROM Catalog.X").unwrap_err();
    assert!(error.line() >= 1);
    let source = format!("SELECT {}1{}", "(".repeat(80), ")".repeat(80));
    assert!(query_metadata_references(&source).is_err());
}

#[test]
fn direct_discovery_accepts_the_existing_binary_operator_budget_boundary() {
    let source = format!("SELECT {}", vec!["TYPE(Catalog.Target)"; 4097].join("+"));
    let references = query_metadata_references(&source).unwrap();
    assert_eq!(references.objects.len(), 1);
}
